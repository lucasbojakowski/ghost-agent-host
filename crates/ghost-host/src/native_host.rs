//! Child-facing CLAP host callbacks and their bounded handoff to the outer host.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::ThreadId;
use std::time::{Duration, Instant};

use clack_extensions::audio_ports::{
    AudioPortRescanFlags, HostAudioPorts, HostAudioPortsImpl,
};
use clack_extensions::gui::{GuiSize, HostGui, HostGuiImpl};
use clack_extensions::latency::{HostLatency, HostLatencyImpl};
use clack_extensions::log::{HostLog, HostLogImpl, LogSeverity};
use clack_extensions::params::{
    HostParams, HostParamsImplMainThread, HostParamsImplShared, ParamClearFlags, ParamRescanFlags,
};
use clack_extensions::state::{HostState, HostStateImpl};
use clack_extensions::thread_check::{HostThreadCheck, HostThreadCheckImpl};
use clack_extensions::timer::{HostTimer, HostTimerImpl, TimerId};
use clack_host::prelude::{
    HostError, HostExtensions, HostHandlers, MainThreadHandler, SharedHandler,
};
use clack_host::utils::ClapId;
use crossbeam_queue::ArrayQueue;

const EVENT_CAPACITY: usize = 256;

thread_local! {
    /// CLAP's audio/main distinction is an execution domain, not a permanent OS-thread identity.
    /// A nested plugin may be started/stopped from the outer main OS thread as long as that call is
    /// executed under audio-domain guarantees. Track that scope per calling thread rather than via
    /// a global "some processor is active" bit.
    static AUDIO_DOMAIN_DEPTH: Cell<u32> = const { Cell::new(0) };
}

pub(crate) fn with_audio_thread_scope<R>(function: impl FnOnce() -> R) -> R {
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            AUDIO_DOMAIN_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
        }
    }

    AUDIO_DOMAIN_DEPTH.with(|depth| depth.set(depth.get().saturating_add(1)));
    let _guard = Guard;
    function()
}

fn in_audio_thread_scope() -> bool {
    AUDIO_DOMAIN_DEPTH.with(|depth| depth.get() > 0)
}

#[derive(Debug, Clone, PartialEq)]
pub enum NestedHostEvent {
    GuiResizeHintsChanged,
    GuiResizeRequested(GuiSize),
    GuiShowRequested,
    GuiHideRequested,
    GuiClosed { was_destroyed: bool },
    ParametersRescan { flags: u32 },
    ParameterClear { parameter_id: u32, flags: u32 },
    ParameterValue { parameter_id: u32, value: f64 },
    StateDirty,
    LatencyChanged,
    Log { severity: String, message: String },
}

/// Routes core wakeups from a child instance to the outer CLAP host without introducing a
/// dependency from `ghost-host` back to the outer plugin adapter.
pub trait NestedHostBridge: Send + Sync {
    fn request_restart(&self) {}
    fn request_process(&self) {}
    fn request_params_flush(&self) {
        self.request_process();
    }
    fn request_main_thread(&self) {}
}

#[derive(Default)]
pub struct NoopNestedHostBridge;
impl NestedHostBridge for NoopNestedHostBridge {}

pub(crate) struct NativeHost;

pub(crate) struct NativeHostShared {
    bridge: Arc<dyn NestedHostBridge>,
    events: ArrayQueue<NestedHostEvent>,
    callback_requested: AtomicBool,
    flush_requested: AtomicBool,
    main_thread_id: ThreadId,
}

impl NativeHostShared {
    pub(crate) fn new(bridge: Arc<dyn NestedHostBridge>) -> Self {
        Self {
            bridge,
            events: ArrayQueue::new(EVENT_CAPACITY),
            callback_requested: AtomicBool::new(false),
            flush_requested: AtomicBool::new(false),
            main_thread_id: std::thread::current().id(),
        }
    }

    fn push(&self, event: NestedHostEvent) {
        let _ = self.events.push(event);
        self.bridge.request_main_thread();
    }

    pub(crate) fn drain_events(&self, output: &mut Vec<NestedHostEvent>) {
        while let Some(event) = self.events.pop() {
            output.push(event);
        }
    }

    pub(crate) fn take_callback_request(&self) -> bool {
        self.callback_requested.swap(false, Ordering::AcqRel)
    }

    pub(crate) fn take_flush_request(&self) -> bool {
        self.flush_requested.swap(false, Ordering::AcqRel)
    }

    pub(crate) fn parameter_feedback(&self, parameter_id: u32, value: f64) {
        self.push(NestedHostEvent::ParameterValue {
            parameter_id,
            value,
        });
    }
}

impl SharedHandler<'_> for NativeHostShared {
    fn request_restart(&self) {
        self.bridge.request_restart();
    }

    fn request_process(&self) {
        self.bridge.request_process();
    }

    fn request_callback(&self) {
        self.callback_requested.store(true, Ordering::Release);
        self.bridge.request_main_thread();
    }
}

impl HostGuiImpl for NativeHostShared {
    fn resize_hints_changed(&self) {
        self.push(NestedHostEvent::GuiResizeHintsChanged);
    }

    fn request_resize(&self, new_size: GuiSize) -> Result<(), HostError> {
        if new_size.width == 0 || new_size.height == 0 {
            return Err(HostError::Message("child requested an empty GUI size"));
        }
        self.push(NestedHostEvent::GuiResizeRequested(new_size));
        Ok(())
    }

    fn request_show(&self) -> Result<(), HostError> {
        self.push(NestedHostEvent::GuiShowRequested);
        Ok(())
    }

    fn request_hide(&self) -> Result<(), HostError> {
        self.push(NestedHostEvent::GuiHideRequested);
        Ok(())
    }

    fn closed(&self, was_destroyed: bool) {
        self.push(NestedHostEvent::GuiClosed { was_destroyed });
    }
}

impl HostParamsImplShared for NativeHostShared {
    fn request_flush(&self) {
        self.flush_requested.store(true, Ordering::Release);
        self.bridge.request_params_flush();
    }
}

impl HostThreadCheckImpl for NativeHostShared {
    fn is_main_thread(&self) -> bool {
        !in_audio_thread_scope() && std::thread::current().id() == self.main_thread_id
    }

    fn is_audio_thread(&self) -> bool {
        in_audio_thread_scope()
    }
}

impl HostLogImpl for NativeHostShared {
    fn log(&self, severity: LogSeverity, message: &str) {
        // Never allocate or forward diagnostics from an audio execution domain.
        if in_audio_thread_scope() {
            return;
        }
        self.push(NestedHostEvent::Log {
            severity: format!("{severity:?}"),
            message: message.to_owned(),
        });
    }
}

pub(crate) struct NativeHostMain<'a> {
    shared: &'a NativeHostShared,
    timers: BTreeMap<TimerId, TimerRegistration>,
    next_timer_id: u32,
}

struct TimerRegistration {
    period: Duration,
    next_tick: Instant,
    stop: Arc<AtomicBool>,
}

impl<'a> NativeHostMain<'a> {
    pub(crate) fn new(shared: &'a NativeHostShared) -> Self {
        Self {
            shared,
            timers: BTreeMap::new(),
            next_timer_id: 1,
        }
    }

    pub(crate) fn due_timers(&mut self, now: Instant, output: &mut Vec<TimerId>) {
        for (id, timer) in &mut self.timers {
            if now >= timer.next_tick {
                output.push(*id);
                timer.next_tick = now + timer.period;
            }
        }
    }
}

impl<'a> MainThreadHandler<'a> for NativeHostMain<'a> {}

impl HostParamsImplMainThread for NativeHostMain<'_> {
    fn rescan(&mut self, flags: ParamRescanFlags) {
        self.shared.push(NestedHostEvent::ParametersRescan {
            flags: flags.bits(),
        });
    }

    fn clear(&mut self, param_id: ClapId, flags: ParamClearFlags) {
        self.shared.push(NestedHostEvent::ParameterClear {
            parameter_id: param_id.get(),
            flags: flags.bits(),
        });
    }
}

impl HostAudioPortsImpl for NativeHostMain<'_> {
    fn is_rescan_flag_supported(&self, _flag: AudioPortRescanFlags) -> bool {
        true
    }

    fn rescan(&mut self, flags: AudioPortRescanFlags) {
        // Port topology is discovered again whenever the outer graph reactivates the child.
        // Structural rescan flags therefore translate directly into a graph restart. Name-only
        // changes do not affect Ghost's routing and need no additional main-thread event.
        if flags.requires_deactivate() {
            self.shared.bridge.request_restart();
        }
    }
}

impl HostStateImpl for NativeHostMain<'_> {
    fn mark_dirty(&mut self) {
        self.shared.push(NestedHostEvent::StateDirty);
    }
}

impl HostLatencyImpl for NativeHostMain<'_> {
    fn changed(&mut self) {
        self.shared.push(NestedHostEvent::LatencyChanged);
    }
}

impl HostTimerImpl for NativeHostMain<'_> {
    fn register_timer(&mut self, period_ms: u32) -> Result<TimerId, HostError> {
        let period = Duration::from_millis(u64::from(period_ms.max(1)));
        let id = TimerId(self.next_timer_id);
        self.next_timer_id = self.next_timer_id.saturating_add(1).max(1);
        let stop = Arc::new(AtomicBool::new(false));
        let timer_stop = Arc::clone(&stop);
        let bridge = Arc::clone(&self.shared.bridge);
        std::thread::spawn(move || {
            while !timer_stop.load(Ordering::Acquire) {
                std::thread::park_timeout(period);
                if !timer_stop.load(Ordering::Acquire) {
                    bridge.request_main_thread();
                }
            }
        });
        self.timers.insert(
            id,
            TimerRegistration {
                period,
                next_tick: Instant::now() + period,
                stop,
            },
        );
        self.shared.bridge.request_main_thread();
        Ok(id)
    }

    fn unregister_timer(&mut self, timer_id: TimerId) -> Result<(), HostError> {
        self.timers
            .remove(&timer_id)
            .map(|timer| timer.stop.store(true, Ordering::Release))
            .ok_or(HostError::Message("unknown child timer"))
    }
}

impl Drop for NativeHostMain<'_> {
    fn drop(&mut self) {
        for timer in self.timers.values() {
            timer.stop.store(true, Ordering::Release);
        }
    }
}

impl HostHandlers for NativeHost {
    type Shared<'a> = NativeHostShared;
    type MainThread<'a> = NativeHostMain<'a>;
    type AudioProcessor<'a> = ();

    fn declare_extensions(builder: &mut HostExtensions<Self>, _shared: &Self::Shared<'_>) {
        builder.register::<HostGui>();
        builder.register::<HostParams>();
        builder.register::<HostAudioPorts>();
        builder.register::<HostState>();
        builder.register::<HostLatency>();
        builder.register::<HostTimer>();
        builder.register::<HostThreadCheck>();
        builder.register::<HostLog>();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use super::*;

    #[derive(Default)]
    struct ProbeBridge {
        restart: AtomicUsize,
        process: AtomicUsize,
        flush: AtomicUsize,
        main: AtomicUsize,
    }

    impl NestedHostBridge for ProbeBridge {
        fn request_restart(&self) {
            self.restart.fetch_add(1, Ordering::Relaxed);
        }

        fn request_process(&self) {
            self.process.fetch_add(1, Ordering::Relaxed);
        }

        fn request_params_flush(&self) {
            self.flush.fetch_add(1, Ordering::Relaxed);
        }

        fn request_main_thread(&self) {
            self.main.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn core_gui_params_and_state_callbacks_cross_the_bounded_bridge() {
        let bridge = Arc::new(ProbeBridge::default());
        let shared = NativeHostShared::new(bridge.clone());
        shared.request_restart();
        shared.request_process();
        shared.request_callback();
        HostParamsImplShared::request_flush(&shared);
        shared.request_show().unwrap();
        shared
            .request_resize(GuiSize {
                width: 640,
                height: 480,
            })
            .unwrap();

        let mut main = NativeHostMain::new(&shared);
        main.mark_dirty();
        main.changed();

        let mut events = Vec::new();
        shared.drain_events(&mut events);
        assert_eq!(bridge.restart.load(Ordering::Relaxed), 1);
        assert_eq!(bridge.process.load(Ordering::Relaxed), 1);
        assert_eq!(bridge.flush.load(Ordering::Relaxed), 1);
        assert!(bridge.main.load(Ordering::Relaxed) >= 4);
        assert!(events.contains(&NestedHostEvent::GuiShowRequested));
        assert!(events.contains(&NestedHostEvent::StateDirty));
        assert!(events.contains(&NestedHostEvent::LatencyChanged));
        assert!(shared.take_callback_request());
        assert!(shared.take_flush_request());
    }

    #[test]
    fn thread_check_reports_scoped_audio_domain() {
        let shared = NativeHostShared::new(Arc::new(NoopNestedHostBridge));
        assert!(HostThreadCheckImpl::is_main_thread(&shared));
        assert!(!HostThreadCheckImpl::is_audio_thread(&shared));
        with_audio_thread_scope(|| {
            assert!(!HostThreadCheckImpl::is_main_thread(&shared));
            assert!(HostThreadCheckImpl::is_audio_thread(&shared));
        });
        assert!(HostThreadCheckImpl::is_main_thread(&shared));
    }

    #[test]
    fn structural_audio_port_rescan_requests_graph_restart() {
        let bridge = Arc::new(ProbeBridge::default());
        let shared = NativeHostShared::new(bridge.clone());
        let mut main = NativeHostMain::new(&shared);

        HostAudioPortsImpl::rescan(&mut main, AudioPortRescanFlags::NAMES);
        assert_eq!(bridge.restart.load(Ordering::Relaxed), 0);

        HostAudioPortsImpl::rescan(&mut main, AudioPortRescanFlags::CHANNEL_COUNT);
        assert_eq!(bridge.restart.load(Ordering::Relaxed), 1);
    }
}
