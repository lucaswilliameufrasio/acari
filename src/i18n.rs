#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Language {
    Portuguese,
    English,
}

pub fn detect_language() -> Language {
    for var in &["LANG", "LC_ALL", "LC_MESSAGES"] {
        if let Ok(val) = std::env::var(var) {
            let lower = val.to_lowercase();
            if lower.starts_with("pt") {
                return Language::Portuguese;
            }
        }
    }
    Language::English
}

pub mod msg {
    use super::Language;

    fn s(pt: &'static str, en: &'static str, lang: Language) -> &'static str {
        match lang {
            Language::Portuguese => pt,
            Language::English => en,
        }
    }

    pub fn scan_progress(lang: Language) -> &'static str {
        s(
            "[progresso] {name}: {size}, {files} arquivos",
            "[progress] {name}: {size}, {files} files",
            lang,
        )
    }

    pub fn target_done(lang: Language) -> &'static str {
        s(
            "[concluído] {name}: {size} em {files} arquivos",
            "[done] {name}: {size} across {files} files",
            lang,
        )
    }

    pub fn scan_finished(lang: Language) -> &'static str {
        s(
            "\nScan finalizado. Total recuperável: {total}",
            "\nScan finished. Total reclaimable bytes: {total}",
            lang,
        )
    }

    pub fn no_targets_matched(lang: Language) -> &'static str {
        s(
            "Nenhum alvo corresponde aos filtros.",
            "No scan targets matched your filters.",
            lang,
        )
    }

    pub fn start_cleaning(lang: Language) -> &'static str {
        s(
            "Iniciando limpeza de {n} alvo(s)...",
            "Starting clean for {n} target(s)...",
            lang,
        )
    }

    pub fn start_dry_run(lang: Language) -> &'static str {
        s(
            "Iniciando simulação de limpeza para {n} alvo(s)...",
            "Starting dry-run clean for {n} target(s)...",
            lang,
        )
    }

    pub fn target_cleaned(lang: Language) -> &'static str {
        s(
            "[{mode}] {name}: recuperados={reclaimed} removidos={removed} erros={errors}",
            "[{mode}] {name}: reclaimed={reclaimed} removed={removed} errors={errors}",
            lang,
        )
    }

    pub fn target_cleaned_with_errors(lang: Language) -> &'static str {
        s(
            "[{mode}] {name}: recuperados={reclaimed} removidos={removed} erros={errors} (falhas de permissão ou exclusão detectadas)",
            "[{mode}] {name}: reclaimed={reclaimed} removed={removed} errors={errors} (permission or deletion failures detected)",
            lang,
        )
    }

    pub fn cleaning_finished(lang: Language) -> &'static str {
        s(
            "Limpeza concluída. Alvos: {n}, recuperados: {size}, erros: {errors}",
            "Cleaning finished. Targets: {n}, reclaimed bytes: {size}, errors: {errors}",
            lang,
        )
    }

    pub fn dry_run_finished(lang: Language) -> &'static str {
        s(
            "Simulação concluída. Alvos: {n}, recuperados: {size}, erros: {errors}",
            "Dry-run cleaning finished. Targets: {n}, reclaimed bytes: {size}, errors: {errors}",
            lang,
        )
    }

    pub fn safety_refuse(lang: Language) -> &'static str {
        s(
            "Limpeza destrutiva recusada sem --yes. Use --clean --yes para prosseguir, ou --clean --dry-run.",
            "Refusing destructive clean without --yes. Use --clean --yes to proceed, or --clean --dry-run.",
            lang,
        )
    }

    pub fn tui_title(lang: Language) -> &'static str {
        s(
            "Acari Cleaner | Scanner de cache para macOS/Linux",
            "Acari Cleaner | macOS/Linux cache scanner",
            lang,
        )
    }

    pub fn tui_scanning_status(lang: Language) -> &'static str {
        s(
            "Escaneando em segundo plano...",
            "Scanning in background...",
            lang,
        )
    }

    pub fn tui_ready_status(lang: Language) -> &'static str {
        s(
            "Scan finalizado. Use ↑/↓, espaço para marcar e Enter para limpar.",
            "Scan finished. Use arrows, space to select, enter to clean.",
            lang,
        )
    }

    pub fn tui_no_selection(lang: Language) -> &'static str {
        s(
            "Nenhum alvo selecionado para limpeza.",
            "No targets selected for cleaning.",
            lang,
        )
    }

    pub fn tui_cleaning_status(lang: Language) -> &'static str {
        s(
            "Limpando alvos selecionados...",
            "Cleaning selected targets...",
            lang,
        )
    }

    pub fn tui_finished_status(lang: Language) -> &'static str {
        s(
            "Limpeza concluída: {done} alvos, {reclaimed} recuperados, {errors} erros.",
            "Cleaning finished: {done} targets, {reclaimed} reclaimed, {errors} errors.",
            lang,
        )
    }

    pub fn tui_cancel_hint(lang: Language) -> &'static str {
        s(
            "Pressione q para cancelar o scan",
            "Press q to cancel scan",
            lang,
        )
    }

    pub fn tui_rescan_hint(lang: Language) -> &'static str {
        s("Pressione r para re-escanear", "Press r to rescan", lang)
    }

    pub fn tui_cancelled_status(lang: Language) -> &'static str {
        s(
            "Scan cancelado pelo usuário.",
            "Scan cancelled by user.",
            lang,
        )
    }

    pub fn scanning_progress(lang: Language) -> &'static str {
        s("Escaneando {n}/{total}", "Scanning {n}/{total}", lang)
    }

    pub fn scan_done_progress(lang: Language) -> &'static str {
        s("Scan concluído: {size}", "Scan done: {size}", lang)
    }

    pub fn cleaning_progress(_lang: Language) -> &'static str {
        "Cleaning selected targets"
    }

    pub fn cleaning_finished_progress(lang: Language) -> &'static str {
        s("Limpeza concluída", "Cleaning finished", lang)
    }

    pub fn target_added(lang: Language) -> &'static str {
        s("Alvo '{name}' adicionado.", "Target '{name}' added.", lang)
    }

    pub fn target_add_duplicate(lang: Language) -> &'static str {
        s(
            "Alvo '{name}' já existe. Use 'acari target remove' primeiro.",
            "Target '{name}' already exists. Use 'acari target remove' first.",
            lang,
        )
    }

    pub fn target_removed(lang: Language) -> &'static str {
        s("Alvo '{name}' removido.", "Target '{name}' removed.", lang)
    }

    pub fn target_not_found(lang: Language) -> &'static str {
        s(
            "Alvo '{name}' não encontrado na configuração.",
            "Target '{name}' not found in config.",
            lang,
        )
    }

    pub fn target_list_header(lang: Language) -> &'static str {
        s("Alvos personalizados:", "Custom targets:", lang)
    }

    pub fn target_list_empty(lang: Language) -> &'static str {
        s(
            "Nenhum alvo personalizado configurado. Use 'acari target add' para adicionar um.",
            "No custom targets configured. Use 'acari target add' to add one.",
            lang,
        )
    }

    pub fn target_list_builtin(lang: Language) -> &'static str {
        s("(embutido)", "(built-in)", lang)
    }

    pub fn target_list_custom(lang: Language) -> &'static str {
        s("(personalizado)", "(custom)", lang)
    }

    pub fn config_last_modified(lang: Language) -> &'static str {
        s(
            "Config modificado pela última vez em: {time}",
            "Config last modified: {time}",
            lang,
        )
    }

    pub fn config_updated_at(lang: Language) -> &'static str {
        s(
            "Config atualizado em: {time}",
            "Config updated at: {time}",
            lang,
        )
    }

    pub fn custom_targets_loaded(lang: Language) -> &'static str {
        s(
            "{n} alvo(s) personalizado(s) carregados do config (modificado em: {time})",
            "{n} custom target(s) loaded from config (modified: {time})",
            lang,
        )
    }

    pub fn target_path(lang: Language) -> &'static str {
        s("caminho: ", "path: ", lang)
    }

    pub fn target_desc(lang: Language) -> &'static str {
        s("desc: ", "desc: ", lang)
    }

    pub fn mode_execute(lang: Language) -> &'static str {
        s("EXECUÇÃO", "EXECUTE", lang)
    }

    pub fn mode_dry_run(lang: Language) -> &'static str {
        s("SIMULAÇÃO", "DRY-RUN", lang)
    }

    pub fn sorted_by_size(lang: Language) -> &'static str {
        s("Ordenado por tamanho", "Sorted by size", lang)
    }

    pub fn confirm_prompt(lang: Language) -> &'static str {
        s(
            "Pressione y para confirmar ou n para cancelar",
            "Press y to confirm or n to cancel",
            lang,
        )
    }

    pub fn confirm_clean(lang: Language) -> &'static str {
        s(
            "Confirmar limpeza de {n} alvo(s) (total: {size})? [y/n]",
            "Confirm clean {n} target(s) (total: {size})? [y/n]",
            lang,
        )
    }

    pub fn panel_status(_lang: Language) -> &'static str {
        "Status"
    }

    pub fn panel_progress(_lang: Language) -> &'static str {
        "Progress"
    }

    pub fn panel_targets(lang: Language) -> &'static str {
        s(
            "Alvos (espaço: marcar, a: todos, s: ordenar, enter: limpar)",
            "Targets (space: toggle, a: all, s: sort, enter: clean)",
            lang,
        )
    }

    pub fn panel_footer(lang: Language) -> &'static str {
        s("Rodapé", "Footer", lang)
    }

    pub fn distro_info(lang: Language) -> &'static str {
        s("Detectado: {os}", "Detected: {os}", lang)
    }

    pub fn history_entry_cleaned(_lang: Language) -> &'static str {
        "Clean completed"
    }

    pub fn clean_execute_label(lang: Language) -> &'static str {
        s("limpo", "cleaned", lang)
    }

    pub fn clean_dry_run_label(lang: Language) -> &'static str {
        s("simulação", "dry-run", lang)
    }

    // --- project scan ---

    pub fn builtin_patterns_header(_lang: Language) -> &'static str {
        "Built-in patterns:"
    }

    pub fn custom_patterns_header(_lang: Language) -> &'static str {
        "Custom patterns:"
    }

    pub fn no_custom_patterns(_lang: Language) -> &'static str {
        "No custom patterns configured."
    }

    pub fn pattern_count(lang: Language) -> &'static str {
        s("{n} padrões no total", "{n} total built-in patterns", lang)
    }

    pub fn roots_header(_lang: Language) -> &'static str {
        "Project roots:"
    }

    pub fn no_roots_configured(lang: Language) -> &'static str {
        s(
            "Nenhum root de projeto configurado.",
            "No project roots configured.",
            lang,
        )
    }

    pub fn root_already_exists(lang: Language) -> &'static str {
        s(
            "Root '{path}' já existe.",
            "Root '{path}' already exists.",
            lang,
        )
    }

    pub fn root_added(lang: Language) -> &'static str {
        s("Root '{path}' adicionado.", "Root '{path}' added.", lang)
    }

    pub fn root_removed(lang: Language) -> &'static str {
        s("Root '{path}' removido.", "Root '{path}' removed.", lang)
    }

    pub fn root_not_found(lang: Language) -> &'static str {
        s(
            "Root '{path}' não encontrado.",
            "Root '{path}' not found.",
            lang,
        )
    }

    pub fn root_empty(lang: Language) -> &'static str {
        s("Caminho não pode ser vazio.", "Path cannot be empty.", lang)
    }

    pub fn pattern_is_builtin(lang: Language) -> &'static str {
        s(
            "'{pattern}' já é um padrão embutido.",
            "'{pattern}' is a built-in pattern.",
            lang,
        )
    }

    pub fn pattern_exists(lang: Language) -> &'static str {
        s(
            "Padrão '{pattern}' já existe nos custom patterns.",
            "Pattern '{pattern}' already added as custom.",
            lang,
        )
    }

    pub fn pattern_added(lang: Language) -> &'static str {
        s(
            "Padrão '{pattern}' adicionado.",
            "Pattern '{pattern}' added.",
            lang,
        )
    }

    pub fn pattern_removed(lang: Language) -> &'static str {
        s(
            "Padrão '{pattern}' removido.",
            "Pattern '{pattern}' removed.",
            lang,
        )
    }

    pub fn pattern_not_found(lang: Language) -> &'static str {
        s(
            "Padrão '{pattern}' não encontrado.",
            "Pattern '{pattern}' not found.",
            lang,
        )
    }

    pub fn patterns_cleared(lang: Language) -> &'static str {
        s(
            "{n} padrão(s) custom removido(s).",
            "{n} custom pattern(s) removed.",
            lang,
        )
    }

    pub fn no_junk_found(lang: Language) -> &'static str {
        s(
            "Nenhum diretório de lixo encontrado.",
            "No junk directories found.",
            lang,
        )
    }

    pub fn junk_found(lang: Language) -> &'static str {
        s(
            "{n} diretório(s) de lixo encontrados.",
            "Found {n} junk directory(ies).",
            lang,
        )
    }

    // --- project TUI ---

    pub fn project_tui_title(lang: Language) -> &'static str {
        s(
            "Acari — Gerenciador de Lixo de Projetos",
            "Acari — Project Junk Manager",
            lang,
        )
    }

    pub fn project_panel_title(lang: Language) -> &'static str {
        s("Gerenciador", "Manager", lang)
    }

    pub fn project_builtin_label(lang: Language) -> &'static str {
        s("Embutidos", "Built-in", lang)
    }

    pub fn project_patterns(_lang: Language) -> &'static str {
        "patterns"
    }

    pub fn project_custom_label(lang: Language) -> &'static str {
        s("Personalizados", "Custom", lang)
    }

    pub fn project_patterns_title(lang: Language) -> &'static str {
        s("Padrões de Diretórios", "Directory Patterns", lang)
    }

    pub fn project_roots_title(lang: Language) -> &'static str {
        s("Roots de Projetos", "Project Roots", lang)
    }

    pub fn project_action_scan(lang: Language) -> &'static str {
        s("Escanear", "Scan", lang)
    }

    pub fn project_action_dry_run(lang: Language) -> &'static str {
        s("Simular", "Dry-run check", lang)
    }

    pub fn project_action_add_pattern(lang: Language) -> &'static str {
        s("Add pattern", "Add pattern", lang)
    }

    pub fn project_action_add_root(lang: Language) -> &'static str {
        s("Add root", "Add root", lang)
    }

    pub fn project_action_remove(lang: Language) -> &'static str {
        s("Remover selecionado", "Remove selected", lang)
    }

    pub fn project_action_switch(lang: Language) -> &'static str {
        s("Alternar foco", "Switch focus", lang)
    }

    pub fn project_action_quit(lang: Language) -> &'static str {
        s("Sair", "Quit", lang)
    }

    pub fn project_actions_title(lang: Language) -> &'static str {
        s("Ações", "Actions", lang)
    }

    pub fn project_prompt_pattern(lang: Language) -> &'static str {
        s("Digite o nome do padrão:", "Enter pattern name:", lang)
    }

    pub fn project_prompt_root(lang: Language) -> &'static str {
        s("Digite o caminho do root:", "Enter root path:", lang)
    }

    pub fn project_empty_pattern(lang: Language) -> &'static str {
        s(
            "Nome do padrão não pode ser vazio.",
            "Pattern name cannot be empty.",
            lang,
        )
    }

    pub fn starting_scan(lang: Language) -> &'static str {
        s("Iniciando scan...", "Starting scan...", lang)
    }

    pub fn starting_dry_run(lang: Language) -> &'static str {
        s("Iniciando simulação...", "Starting dry-run...", lang)
    }

    pub fn project_hint_add_root(lang: Language) -> &'static str {
        s("Pressione [p] para adicionar", "Press [p] to add one", lang)
    }

    pub fn project_hint_add_pattern(lang: Language) -> &'static str {
        s(
            "Nenhum padrão custom — pressione [a] para adicionar ou use os built-in",
            "No custom patterns — press [a] to add or use the built-in ones",
            lang,
        )
    }

    pub fn pattern_invalid_name(lang: Language) -> &'static str {
        s(
            "Nome inválido: não pode conter /, \\, .., e deve ter no máximo 64 caracteres.",
            "Invalid name: no /, \\, .. allowed, max 64 chars.",
            lang,
        )
    }

    pub fn confirm_remove_pattern(lang: Language) -> &'static str {
        s(
            "Remover '{pattern}'? [y/n]",
            "Remove '{pattern}'? [y/n]",
            lang,
        )
    }

    pub fn pattern_layout_label(lang: Language) -> &'static str {
        s("embutido", "built-in", lang)
    }

    pub fn custom_layout_label(lang: Language) -> &'static str {
        s("custom", "custom", lang)
    }
}
