use std::collections::BTreeSet;

use serde::Serialize;
use thiserror::Error;

const ENRICHED_SIGNATURES: &str = include_str!(
    "../../../docs/daw-apis/fl-studio/fl_studio_api_dump.enriched.signatures"
);

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FlScriptingFunction {
    pub module: String,
    pub function: String,
    pub signature: Option<String>,
    pub arguments: Option<String>,
    pub returns: Option<String>,
    pub description: Option<String>,
    pub minimum_api_version: Option<u32>,
    pub api_version: Option<String>,
    pub bridge_callable: bool,
    pub unsupported_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FlScriptingCatalog {
    functions: Vec<FlScriptingFunction>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlScriptingManifest {
    pub bridge: &'static str,
    pub protocol: u32,
    pub fl_version: Option<String>,
    pub scripting_api_version: Option<i64>,
    pub functions: Vec<FlScriptingManifestFunction>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FlScriptingManifestFunction {
    pub module: String,
    pub function: String,
    pub signature: Option<String>,
    pub returns: Option<String>,
    pub description: Option<String>,
    pub minimum_api_version: Option<u32>,
    pub api_version: Option<String>,
    pub bridge_callable: bool,
    pub available_in_connected_api: Option<bool>,
    pub unsupported_reason: Option<String>,
}

#[derive(Debug, Error)]
pub(crate) enum CatalogError {
    #[error("FL scripting catalog could not be parsed: {0}")]
    Parse(String),
}

impl FlScriptingCatalog {
    pub fn bundled() -> Result<Self, String> {
        Self::parse(ENRICHED_SIGNATURES).map_err(|error| error.to_string())
    }

    pub fn functions(&self) -> &[FlScriptingFunction] {
        &self.functions
    }

    pub fn modules(&self) -> Vec<&str> {
        self.functions
            .iter()
            .map(|function| function.module.as_str())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn describe(&self, module: &str, function: &str) -> Vec<&FlScriptingFunction> {
        self.functions
            .iter()
            .filter(|entry| entry.module == module && entry.function == function)
            .collect()
    }

    pub(crate) fn ensure_bridge_callable(
        &self,
        module: &str,
        function: &str,
    ) -> Result<(), String> {
        let matches = self.describe(module, function);
        if matches.is_empty() {
            return Err(format!(
                "FL scripting function `{module}.{function}` is absent from the checked-in runtime metadata"
            ));
        }
        if matches.iter().any(|entry| entry.bridge_callable) {
            return Ok(());
        }
        let reason = matches
            .iter()
            .find_map(|entry| entry.unsupported_reason.as_deref())
            .unwrap_or("the checked-in metadata does not establish a JSON-compatible call shape");
        Err(format!(
            "FL scripting function `{module}.{function}` is not callable through the JSON bridge: {reason}"
        ))
    }

    pub(crate) fn manifest_functions(
        &self,
        scripting_api_version: Option<i64>,
    ) -> Vec<FlScriptingManifestFunction> {
        self.functions
            .iter()
            .map(|entry| FlScriptingManifestFunction {
                module: entry.module.clone(),
                function: entry.function.clone(),
                signature: entry.signature.clone(),
                returns: entry.returns.clone(),
                description: entry.description.clone(),
                minimum_api_version: entry.minimum_api_version,
                api_version: entry.api_version.clone(),
                bridge_callable: entry.bridge_callable,
                available_in_connected_api: scripting_api_version.map(|connected| {
                    entry
                        .minimum_api_version
                        .is_none_or(|minimum| connected >= i64::from(minimum))
                }),
                unsupported_reason: entry.unsupported_reason.clone(),
            })
            .collect()
    }

    fn parse(source: &str) -> Result<Self, CatalogError> {
        let mut functions = Vec::new();
        let mut current_module: Option<String> = None;
        let mut pending: Option<PendingFunction> = None;
        let mut section = Section::Documented;

        for raw_line in source.lines() {
            let line = raw_line.trim_end();
            if line == "RUNTIME-INSPECTED SIGNATURES" {
                flush_pending(&mut functions, &mut pending)?;
                section = Section::RuntimeInspected;
                current_module = None;
                continue;
            }
            if line == "FUNCTIONS WITH NO SIGNATURE METADATA" {
                flush_pending(&mut functions, &mut pending)?;
                section = Section::NoSignature;
                current_module = None;
                continue;
            }
            if line.starts_with("====") || line.is_empty() {
                if matches!(section, Section::Documented) && line.is_empty() {
                    flush_pending(&mut functions, &mut pending)?;
                }
                continue;
            }

            match section {
                Section::Documented => {
                    if let Some(module) = line.strip_prefix("MODULE: ") {
                        flush_pending(&mut functions, &mut pending)?;
                        current_module = Some(module.trim().to_owned());
                        continue;
                    }
                    if let Some(value) = line.strip_prefix("  Arguments:") {
                        if let Some(entry) = pending.as_mut() {
                            entry.arguments = Some(value.trim().to_owned());
                        }
                        continue;
                    }
                    if let Some(value) = line.strip_prefix("  Returns:") {
                        if let Some(entry) = pending.as_mut() {
                            entry.returns = Some(value.trim().to_owned());
                        }
                        continue;
                    }
                    if let Some(value) = line.strip_prefix("  Description:") {
                        if let Some(entry) = pending.as_mut() {
                            entry.description = Some(value.trim().to_owned());
                        }
                        continue;
                    }
                    if let Some(value) = line.strip_prefix("  API version:") {
                        if let Some(entry) = pending.as_mut() {
                            entry.api_version = Some(value.trim().to_owned());
                        }
                        continue;
                    }
                    if line.starts_with(' ') {
                        continue;
                    }
                    let Some(module) = current_module.as_ref() else {
                        continue;
                    };
                    if line.contains('(') && line.contains(')') {
                        flush_pending(&mut functions, &mut pending)?;
                        pending = Some(PendingFunction::new(module.clone(), line)?);
                    }
                }
                Section::RuntimeInspected => {
                    if let Some((module, function, signature)) = parse_runtime_signature(line) {
                        functions.push(FlScriptingFunction {
                            module,
                            function,
                            signature: Some(signature),
                            arguments: None,
                            returns: None,
                            description: None,
                            minimum_api_version: None,
                            api_version: None,
                            bridge_callable: false,
                            unsupported_reason: Some(
                                "runtime inspection did not establish return/API metadata".into(),
                            ),
                        });
                    }
                }
                Section::NoSignature => {
                    if let Some((module, function)) = line.split_once('.') {
                        if is_identifier(module) && is_identifier(function) {
                            functions.push(FlScriptingFunction {
                                module: module.to_owned(),
                                function: function.to_owned(),
                                signature: None,
                                arguments: None,
                                returns: None,
                                description: None,
                                minimum_api_version: None,
                                api_version: None,
                                bridge_callable: false,
                                unsupported_reason: Some(
                                    "no argument/return signature metadata is available".into(),
                                ),
                            });
                        }
                    }
                }
            }
        }
        flush_pending(&mut functions, &mut pending)?;
        if functions.is_empty() {
            return Err(CatalogError::Parse("catalog contained no functions".into()));
        }
        Ok(Self { functions })
    }
}

#[derive(Clone, Copy)]
enum Section {
    Documented,
    RuntimeInspected,
    NoSignature,
}

struct PendingFunction {
    module: String,
    function: String,
    signature: String,
    arguments: Option<String>,
    returns: Option<String>,
    description: Option<String>,
    api_version: Option<String>,
}

impl PendingFunction {
    fn new(module: String, signature: &str) -> Result<Self, CatalogError> {
        let function = signature
            .split_once('(')
            .map(|(function, _)| function.trim())
            .filter(|function| is_identifier(function))
            .ok_or_else(|| CatalogError::Parse(format!("invalid function signature `{signature}`")))?;
        Ok(Self {
            module,
            function: function.to_owned(),
            signature: signature.to_owned(),
            arguments: None,
            returns: None,
            description: None,
            api_version: None,
        })
    }

    fn finish(self) -> FlScriptingFunction {
        let minimum_api_version = self.api_version.as_deref().and_then(first_unsigned_integer);
        let (bridge_callable, unsupported_reason) = if crate::adapter::BRIDGE_MODULES
            .contains(&self.module.as_str())
        {
            bridge_support(
                self.arguments.as_deref().unwrap_or_default(),
                self.returns.as_deref().unwrap_or_default(),
            )
        } else {
            (
                false,
                Some("module is not imported by the current FL scripting bridge".into()),
            )
        };
        FlScriptingFunction {
            module: self.module,
            function: self.function,
            signature: Some(self.signature),
            arguments: self.arguments,
            returns: self.returns,
            description: self.description,
            minimum_api_version,
            api_version: self.api_version,
            bridge_callable,
            unsupported_reason,
        }
    }
}

fn flush_pending(
    functions: &mut Vec<FlScriptingFunction>,
    pending: &mut Option<PendingFunction>,
) -> Result<(), CatalogError> {
    if let Some(entry) = pending.take() {
        if entry.api_version.is_none() {
            return Err(CatalogError::Parse(format!(
                "documented function `{}.{}` is missing API version metadata",
                entry.module, entry.function
            )));
        }
        functions.push(entry.finish());
    }
    Ok(())
}

fn parse_runtime_signature(line: &str) -> Option<(String, String, String)> {
    let (qualified, _) = line.split_once('(')?;
    let (module, function) = qualified.split_once('.')?;
    if !is_identifier(module) || !is_identifier(function) {
        return None;
    }
    Some((module.to_owned(), function.to_owned(), line.to_owned()))
}

fn bridge_support(arguments: &str, returns: &str) -> (bool, Option<String>) {
    let shape = format!("{arguments} {returns}").to_ascii_lowercase();
    for unsupported in ["eventdata", "bytes", "bytearray", "memoryview"] {
        if shape.contains(unsupported) {
            return (
                false,
                Some(format!(
                    "signature contains `{unsupported}`, which the NDJSON primitive-value bridge does not coerce"
                )),
            );
        }
    }
    (true, None)
}

fn first_unsigned_integer(value: &str) -> Option<u32> {
    let digits: String = value
        .chars()
        .skip_while(|character| !character.is_ascii_digit())
        .take_while(|character| character.is_ascii_digit())
        .collect();
    (!digits.is_empty()).then(|| digits.parse().ok()).flatten()
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_checked_in_runtime_catalog() {
        let catalog = FlScriptingCatalog::bundled().unwrap();
        assert!(catalog.functions().len() >= 362);
        let pattern_name = catalog.describe("patterns", "getPatternName");
        assert_eq!(pattern_name.len(), 1);
        assert_eq!(pattern_name[0].minimum_api_version, Some(1));
        assert!(pattern_name[0].bridge_callable);
        assert!(catalog.modules().contains(&"screen"));
        assert!(catalog.modules().contains(&"utils"));
    }

    #[test]
    fn preserves_overloads_and_marks_unsupported_wire_shapes() {
        let catalog = FlScriptingCatalog::bundled().unwrap();
        let midi_out = catalog.describe("device", "midiOutMsg");
        assert_eq!(midi_out.len(), 2);
        assert!(midi_out.iter().all(|entry| entry.bridge_callable));

        let direct_feedback = catalog.describe("device", "directFeedback");
        assert_eq!(direct_feedback.len(), 1);
        assert!(!direct_feedback[0].bridge_callable);
        assert!(direct_feedback[0]
            .unsupported_reason
            .as_deref()
            .unwrap()
            .contains("eventdata"));
    }

    #[test]
    fn unknown_signature_entries_are_discoverable_but_not_callable() {
        let catalog = FlScriptingCatalog::bundled().unwrap();
        let notification = catalog.describe("ui", "showNotification");
        assert_eq!(notification.len(), 1);
        assert!(!notification[0].bridge_callable);
        assert!(notification[0].signature.is_none());
    }
}
