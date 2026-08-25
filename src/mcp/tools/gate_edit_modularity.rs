use std::path::Path;

/// Validates that a new file declaration adheres to single responsibility and upfront modularity,
/// preventing compound/monolithic files before creation regardless of language or framework.
pub(crate) fn validate_new_file_modularity(rel_path: &str, justification: &str) -> Result<(), String> {
    let file_path = Path::new(rel_path);
    let file_name = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let file_stem = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    let clean_stem = file_stem.replace('-', "_");

    // 1. Language-agnostic plural compound container suffixes
    let compound_suffixes = [
        "modals", "dialogs", "drawers", "forms", "tables", "cards", "panels", "widgets",
        "components", "views", "screens", "pages",
        "services", "handlers", "controllers", "managers", "repositories", "endpoints",
        "routes", "actions", "mutations", "queries", "reducers",
        "helpers", "utils", "models", "entities", "adapters", "transformers", "listeners",
    ];

    for suffix in compound_suffixes {
        if clean_stem == suffix || clean_stem.ends_with(&format!("_{}", suffix)) || clean_stem.ends_with(suffix) {
            // Exceptions: single-word specific concepts like 'form' or 'view' (singular) are permitted
            // but compound plurals (e.g. 'inventory_modals', 'UserServices', 'order_handlers') are blocked
            if clean_stem.len() > suffix.len() + 1 {
                return Err(format!(
                    "# Edit Approval Gate: BLOCKED (COMPOUND_FILE_NAME_PROHIBITED)\n\n\
                    - Target File: `{}` [NEW FILE]\n\n\
                    ⚠️ **Error: COMPOUND_FILE_NAME_PROHIBITED**: New file name '{}' is a compound plural container (matches '*{}'). Monolithic container files are strictly forbidden because they bundle multiple components/services and cause 300 LOC limit violations.\n\n\
                    👉 **Action Required**: Authorize and create discrete single-responsibility files (< 150 LOC each), for example:\n\
                    - `CreateItemModal.tsx` and `AdjustStockModal.tsx` instead of `InventoryModals.tsx`\n\
                    - `user_service.rs` and `order_service.rs` instead of `services.rs`\n\
                    - `login_handler.go` and `register_handler.go` instead of `auth_handlers.go`",
                    rel_path, file_name, suffix
                ));
            }
        }
    }

    // 2. Universal multi-component justification detection
    let j_lower = justification.to_lowercase();
    let has_conjunction = j_lower.contains(" and ") || j_lower.contains(" & ") || j_lower.contains(" cùng ") || j_lower.contains(" và ") || j_lower.contains(",");
    let has_action = j_lower.contains("create") || j_lower.contains("add") || j_lower.contains("implement") || j_lower.contains("build") || j_lower.contains("introduce");

    if has_conjunction && has_action {
        let words: Vec<&str> = justification.split(|c: char| c.is_whitespace() || c == ',' || c == '&').filter(|s| !s.trim().is_empty()).collect();
        let entity_count = words.iter().filter(|w| {
            let clean = w.trim_matches(|c: char| !c.is_alphanumeric());
            (clean.ends_with("Modal") || clean.ends_with("Dialog") || clean.ends_with("View") || clean.ends_with("Component")
                || clean.ends_with("Service") || clean.ends_with("Handler") || clean.ends_with("Controller")
                || clean.ends_with("Repo") || clean.ends_with("Repository") || clean.ends_with("Screen")
                || clean.ends_with("Manager") || clean.ends_with("Router") || clean.ends_with("Adapter"))
                && clean.len() > 6
        }).count();

        if entity_count >= 2 {
            return Err(format!(
                "# Edit Approval Gate: BLOCKED (MULTI_COMPONENT_NEW_FILE_PROHIBITED)\n\n\
                - Target File: `{}` [NEW FILE]\n\
                - Justification: {}\n\n\
                ⚠️ **Error: MULTI_COMPONENT_NEW_FILE_PROHIBITED**: Your authorization justification declares {} distinct components/services in a single new file. Packing multiple components into one file violates Single Responsibility and breaches the 300 LOC hard cap.\n\n\
                👉 **Action Required**: Authorize and create EACH component in its own dedicated file (< 150 LOC each):\n\
                - Call `workflow_gate(action=\"authorize_edit\", relative_path=\"<path>/ComponentA\", ...)`\n\
                - Call `workflow_gate(action=\"authorize_edit\", relative_path=\"<path>/ComponentB\", ...)`",
                rel_path, justification, entity_count
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blocks_plural_compound_names_across_languages() {
        // TypeScript / React
        assert!(validate_new_file_modularity("src/components/InventoryModals.tsx", "Modals").is_err());
        assert!(validate_new_file_modularity("src/views/DashboardWidgets.vue", "Widgets").is_err());
        
        // Go
        assert!(validate_new_file_modularity("internal/services/order_services.go", "Order services").is_err());
        assert!(validate_new_file_modularity("internal/handlers/auth_handlers.go", "Auth handlers").is_err());

        // Rust
        assert!(validate_new_file_modularity("src/controllers/user_controllers.rs", "User controllers").is_err());
        assert!(validate_new_file_modularity("src/utils/general_helpers.rs", "Helpers").is_err());

        // Python
        assert!(validate_new_file_modularity("app/api/item_endpoints.py", "Endpoints").is_err());
    }

    #[test]
    fn test_allows_single_responsibility_names() {
        assert!(validate_new_file_modularity("src/components/CreateItemModal.tsx", "Create item modal").is_ok());
        assert!(validate_new_file_modularity("internal/service/order_service.go", "Order service").is_ok());
        assert!(validate_new_file_modularity("src/mcp/tools/gate_edit.rs", "Edit gate handler").is_ok());
        assert!(validate_new_file_modularity("app/api/item_router.py", "Item router").is_ok());
    }

    #[test]
    fn test_blocks_multi_component_justifications() {
        let res = validate_new_file_modularity(
            "src/components/InventoryDialog.tsx",
            "Add CreateItemModal and AdjustStockModal for inventory",
        );
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("MULTI_COMPONENT_NEW_FILE_PROHIBITED"));
    }
}
