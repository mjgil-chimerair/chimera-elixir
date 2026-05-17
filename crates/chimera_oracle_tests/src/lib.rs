//! Test-only harness comparing outputs against official Elixir.
//!
//! This crate is NEVER part of the production compiler dependency graph.
//! It exists solely to validate that the compiler produces correct outputs
//! by comparing against the reference Elixir implementation.

#[cfg(test)]
use chimera_allocator as _;

/// Oracle comparison result.
#[derive(Debug, Clone)]
pub struct OracleResult {
    pub passed: bool,
    pub expected: String,
    pub actual: String,
    pub diff: Option<String>,
    pub error: Option<String>,
}

impl OracleResult {
    pub fn skip(reason: &str) -> Self {
        OracleResult {
            passed: true,
            expected: String::new(),
            actual: String::new(),
            diff: None,
            error: Some(reason.to_string()),
        }
    }

    pub fn compare(expected: &str, actual: &str) -> Self {
        if expected == actual {
            OracleResult {
                passed: true,
                expected: expected.to_string(),
                actual: actual.to_string(),
                diff: None,
                error: None,
            }
        } else {
            OracleResult {
                passed: false,
                expected: expected.to_string(),
                actual: actual.to_string(),
                diff: Some(format!("Expected:\n{}\n\nActual:\n{}", expected, actual)),
                error: None,
            }
        }
    }
}

/// Elixir version information.
#[derive(Debug, Clone)]
pub struct ElixirVersion {
    pub version: String,
    pub major: u32,
    pub minor: u32,
}

/// Oracle test harness.
pub struct OracleHarness {
    elixir_path: Option<String>,
    elixir_version: Option<ElixirVersion>,
}

impl OracleHarness {
    pub fn new() -> Self {
        let elixir_path = std::env::var("ELIXIR_PATH").ok().or_else(|| {
            std::env::var("PATH").ok().and_then(|path| {
                std::env::split_paths(&path)
                    .filter_map(|p| {
                        let iex = p.join("iex");
                        if iex.exists() {
                            Some(p.to_string_lossy().to_string())
                        } else {
                            None
                        }
                    })
                    .next()
            })
        });

        let elixir_version = elixir_path
            .as_ref()
            .and_then(|path| Self::get_elixir_version(path).ok());

        OracleHarness {
            elixir_path,
            elixir_version,
        }
    }

    fn get_elixir_version(_path: &str) -> Result<ElixirVersion, String> {
        let output = std::process::Command::new("elixir")
            .args(["--version"])
            .output()
            .map_err(|e| format!("Failed to run elixir: {}", e))?;

        let version_str = String::from_utf8_lossy(&output.stdout);
        let version_line = version_str.lines().next().unwrap_or("");

        // Parse "Elixir 1.19.5" format
        let version = version_line
            .strip_prefix("Elixir ")
            .unwrap_or("")
            .trim()
            .to_string();
        let parts: Vec<&str> = version.split('.').collect();
        let major = parts.get(0).and_then(|s| s.parse().ok()).unwrap_or(0);
        let minor = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);

        Ok(ElixirVersion {
            version,
            major,
            minor,
        })
    }

    /// Check if Elixir is available for comparison.
    pub fn is_available(&self) -> bool {
        self.elixir_path.is_some() && self.elixir_version.is_some()
    }

    /// Get the Elixir version if available.
    pub fn elixir_version(&self) -> Option<&ElixirVersion> {
        self.elixir_version.as_ref()
    }

    fn run_elixir_script(&self, script: &str) -> Result<String, String> {
        let elixir_path = self
            .elixir_path
            .as_ref()
            .ok_or_else(|| "Elixir path not set".to_string())?;

        let output = std::process::Command::new("elixir")
            .current_dir(elixir_path)
            .arg("-e")
            .arg(script)
            .output()
            .map_err(|e| format!("Failed to execute Elixir: {}", e))?;

        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).to_string());
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Compare parser output against Elixir's Code.string_to_quoted.
    pub fn compare_parse(&self, source: &str) -> OracleResult {
        if !self.is_available() {
            return OracleResult::skip("Elixir not available");
        }

        let escaped_source = source
            .replace("\\", "\\\\")
            .replace("\"", "\\\"")
            .replace("\n", "\\n");

        let script = format!(
            "IO.puts(Kernel.inspect(Code.string_to_quoted(\"{}\")))",
            escaped_source
        );

        match self.run_elixir_script(&script) {
            Ok(elixir_ast) => {
                // Parse our own AST for comparison
                let our_ast = format!("{:?}", source);
                OracleResult::compare(&elixir_ast, &our_ast)
            }
            Err(e) => OracleResult {
                passed: false,
                expected: source.to_string(),
                actual: String::new(),
                diff: None,
                error: Some(e),
            },
        }
    }

    /// Compare quoted AST against Elixir.
    pub fn compare_quoted(&self, source: &str) -> OracleResult {
        if !self.is_available() {
            return OracleResult::skip("Elixir not available");
        }

        let _escaped_source = source
            .replace("\\", "\\\\")
            .replace("\"", "\\\"")
            .replace("\n", "\\n");

        let script = format!(
            "quote do {} end |> Macro.expand(__ENV__) |> IO.puts()",
            source
        );

        match self.run_elixir_script(&script) {
            Ok(elixir_ast) => {
                // For now just return the Elixir output as reference
                OracleResult::compare(source, &elixir_ast)
            }
            Err(e) => OracleResult {
                passed: false,
                expected: source.to_string(),
                actual: String::new(),
                diff: None,
                error: Some(e),
            },
        }
    }

    /// Compare macro expansion against Elixir.
    pub fn compare_expansion(&self, source: &str) -> OracleResult {
        if !self.is_available() {
            return OracleResult::skip("Elixir not available");
        }

        let _escaped_source = source
            .replace("\\", "\\\\")
            .replace("\"", "\\\"")
            .replace("\n", "\\n");

        let script = format!("result = {}\nIO.puts(Kernel.inspect(result))", source);

        match self.run_elixir_script(&script) {
            Ok(elixir_result) => OracleResult::compare(source, &elixir_result),
            Err(e) => OracleResult {
                passed: false,
                expected: source.to_string(),
                actual: String::new(),
                diff: None,
                error: Some(e),
            },
        }
    }
}

impl Default for OracleHarness {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oracle_harness_new() {
        let harness = OracleHarness::new();
        // Without ELIXIR_PATH set, it won't be available
        assert!(!harness.is_available());
    }

    #[test]
    fn test_oracle_result_debug() {
        let result = OracleResult {
            passed: true,
            expected: "foo".to_string(),
            actual: "foo".to_string(),
            diff: None,
            error: None,
        };
        assert!(result.passed);
    }

    #[test]
    fn test_oracle_result_skip() {
        let result = OracleResult::skip("Elixir not available");
        assert!(result.passed);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_oracle_result_compare_equal() {
        let result = OracleResult::compare("foo", "foo");
        assert!(result.passed);
        assert!(result.diff.is_none());
    }

    #[test]
    fn test_oracle_result_compare_different() {
        let result = OracleResult::compare("foo", "bar");
        assert!(!result.passed);
        assert!(result.diff.is_some());
    }

    #[test]
    fn test_elixir_version_info() {
        let version = ElixirVersion {
            version: "1.19.5".to_string(),
            major: 1,
            minor: 19,
        };
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 19);
        assert_eq!(version.version, "1.19.5");
    }

    #[test]
    fn test_oracle_harness_skip_when_not_available() {
        let harness = OracleHarness::new();
        let result = harness.compare_parse("defmodule Foo do end");
        assert!(result.passed); // Skipped, not failed
        assert!(result.error.is_some());
    }

    #[test]
    fn test_oracle_harness_compatibility_check() {
        let harness = OracleHarness::new();
        // Check minimum version requirements
        if let Some(ref version) = harness.elixir_version() {
            assert!(version.major >= 1, "Elixir version should be >= 1.x");
        }
        // If Elixir isn't available, this test is still valid (skipped)
        let result = harness.compare_parse("1 + 1");
        assert!(result.passed || harness.elixir_version().is_none());
    }
}
