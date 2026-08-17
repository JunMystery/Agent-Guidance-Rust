use serde_json::Value;
use super::ServerState;

impl ServerState {
    pub fn can_call_tool(&mut self, tool_name: &str, args: &Value) -> Result<(), String> {
        // 1. Unlocks gate tool and advances stage from Context to Plan
        if tool_name == "task_pipeline" {
            self.priority_gate_pass();
            if self.workflow_stage == "Context" {
                self.workflow_stage = "Plan".to_string();
            }
            return Ok(());
        }

        // 2. Whitelisted & Not Gated tools bypass priority gate check
        let is_whitelisted_or_ungated = matches!(
            tool_name,
            "workflow_gate" | "session_continuity" | "select_skills"
        );

        if !is_whitelisted_or_ungated {
            // Check Layer 2 / Layer 3 Priority Gate
            self.priority_gate_check()?;
        }

        // 3. Perform Stage Checks
        match self.workflow_stage.as_str() {
            "Context" => {
                if !is_whitelisted_or_ungated
                    && tool_name != "workflow_gate"
                    && tool_name != "session_continuity"
                {
                    return Err(format!(
                        "WORKFLOW_STAGE_BLOCKED: Tool '{}' is blocked in 'Context' stage. Call task_pipeline and workflow_gate(action=\"set_stage\", target_stage=\"Plan\") first.",
                        tool_name
                    ));
                }
                Ok(())
            }
            "Plan" => {
                if tool_name == "project_context" {
                    let op = args.get("operation").and_then(|o| o.as_str()).unwrap_or("");
                    if op == "diff" {
                        return Err(format!(
                            "WORKFLOW_STAGE_BLOCKED: Operation '{}' on project_context is blocked in 'Plan' stage.",
                            op
                        ));
                    }
                }
                Ok(())
            }
            "Ask_Revise" => {
                if tool_name == "project_context" {
                    let op = args.get("operation").and_then(|o| o.as_str()).unwrap_or("");
                    if matches!(
                        op,
                        "read"
                            | "search"
                            | "symbols"
                            | "references"
                            | "structure"
                            | "callers"
                            | "callees"
                            | "diff"
                    ) {
                        return Err(format!(
                            "WORKFLOW_STAGE_BLOCKED: Code reading operation '{}' is blocked in 'Ask_Revise' stage.",
                            op
                        ));
                    }
                } else if tool_name == "guidance" {
                    let op = args.get("operation").and_then(|o| o.as_str()).unwrap_or("");
                    if op == "precode" || op == "verify" {
                        return Err(format!(
                            "WORKFLOW_STAGE_BLOCKED: Guidance operation '{}' is blocked in 'Ask_Revise' stage.",
                            op
                        ));
                    }
                }
                Ok(())
            }
            "Build" => {
                if !self.plan_approved {
                    Err("WORKFLOW_STAGE_BLOCKED: Tool execution in 'Build' stage is blocked because plan_approved is false. Obtain user approval first.".to_string())
                } else if tool_name == "workflow_gate"
                    && args.get("action").and_then(|a| a.as_str()) == Some("authorize_edit")
                {
                    let arch_pattern = args
                        .get("architecture_pattern")
                        .and_then(|a| a.as_str())
                        .unwrap_or("");
                    if !matches!(
                        arch_pattern,
                        ""
                            | "Auto"
                            | "auto"
                            | "Clean_Architecture"
                            | "Layered_Architecture"
                            | "Package_By_Feature"
                            | "Orchestrator"
                            | "CLI_Pipeline"
                            | "Flat_Library"
                    ) {
                        Err("ARCHITECTURE_GATE_BLOCKED: You must provide a valid `architecture_pattern` ('Clean_Architecture', 'Layered_Architecture', 'Package_By_Feature', 'Orchestrator', 'CLI_Pipeline', 'Flat_Library', or 'Auto') in `workflow_gate(action=\"authorize_edit\")`.".to_string())
                    } else {
                        Ok(())
                    }
                } else {
                    Ok(())
                }
            }
            "Test_Recheck" => {
                if tool_name == "guidance" {
                    let op = args.get("operation").and_then(|o| o.as_str()).unwrap_or("");
                    if op == "precode" {
                        return Err("WORKFLOW_STAGE_BLOCKED: Operation 'precode' is blocked in 'Test_Recheck' stage.".to_string());
                    }
                }
                Ok(())
            }
            "Fix" => Ok(()),
            "Proposal" => {
                if tool_name == "project_context" {
                    let op = args.get("operation").and_then(|o| o.as_str()).unwrap_or("");
                    if matches!(op, "diff" | "structure" | "symbols") {
                        return Err(format!(
                            "WORKFLOW_STAGE_BLOCKED: Operation '{}' is blocked in 'Proposal' stage.",
                            op
                        ));
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}
