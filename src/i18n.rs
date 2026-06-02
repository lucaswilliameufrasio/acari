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
        s("[progresso] {name}: {size}, {files} arquivos",
          "[progress] {name}: {size}, {files} files", lang)
    }

    pub fn target_done(lang: Language) -> &'static str {
        s("[concluído] {name}: {size} em {files} arquivos",
          "[done] {name}: {size} across {files} files", lang)
    }

    pub fn scan_finished(lang: Language) -> &'static str {
        s("\nScan finalizado. Total recuperável: {total}",
          "\nScan finished. Total reclaimable bytes: {total}", lang)
    }

    pub fn no_targets_matched(lang: Language) -> &'static str {
        s("Nenhum alvo corresponde aos filtros.",
          "No scan targets matched your filters.", lang)
    }

    pub fn start_cleaning(lang: Language) -> &'static str {
        s("Iniciando limpeza de {n} alvo(s)...",
          "Starting clean for {n} target(s)...", lang)
    }

    pub fn start_dry_run(lang: Language) -> &'static str {
        s("Iniciando simulação de limpeza para {n} alvo(s)...",
          "Starting dry-run clean for {n} target(s)...", lang)
    }

    pub fn target_cleaned(lang: Language) -> &'static str {
        s("[{mode}] {name}: recuperados={reclaimed} removidos={removed} erros={errors}",
          "[{mode}] {name}: reclaimed={reclaimed} removed={removed} errors={errors}", lang)
    }

    pub fn target_cleaned_with_errors(lang: Language) -> &'static str {
        s("[{mode}] {name}: recuperados={reclaimed} removidos={removed} erros={errors} (falhas de permissão ou exclusão detectadas)",
          "[{mode}] {name}: reclaimed={reclaimed} removed={removed} errors={errors} (permission or deletion failures detected)", lang)
    }

    pub fn cleaning_finished(lang: Language) -> &'static str {
        s("Limpeza concluída. Alvos: {n}, recuperados: {size}, erros: {errors}",
          "Cleaning finished. Targets: {n}, reclaimed bytes: {size}, errors: {errors}", lang)
    }

    pub fn dry_run_finished(lang: Language) -> &'static str {
        s("Simulação concluída. Alvos: {n}, recuperados: {size}, erros: {errors}",
          "Dry-run cleaning finished. Targets: {n}, reclaimed bytes: {size}, errors: {errors}", lang)
    }

    pub fn safety_refuse(lang: Language) -> &'static str {
        s("Limpeza destrutiva recusada sem --yes. Use --clean --yes para prosseguir, ou --clean --dry-run.",
          "Refusing destructive clean without --yes. Use --clean --yes to proceed, or --clean --dry-run.", lang)
    }

    pub fn tui_title(lang: Language) -> &'static str {
        s("Acari Cleaner | Scanner de cache para macOS/Linux",
          "Acari Cleaner | macOS/Linux cache scanner", lang)
    }

    pub fn tui_scanning_status(lang: Language) -> &'static str {
        s("Escaneando em segundo plano...",
          "Scanning in background...", lang)
    }

    pub fn tui_ready_status(lang: Language) -> &'static str {
        s("Scan finalizado. Use ↑/↓, espaço para marcar e Enter para limpar.",
          "Scan finished. Use arrows, space to select, enter to clean.", lang)
    }

    pub fn tui_no_selection(lang: Language) -> &'static str {
        s("Nenhum alvo selecionado para limpeza.",
          "No targets selected for cleaning.", lang)
    }

    pub fn tui_cleaning_status(lang: Language) -> &'static str {
        s("Limpando alvos selecionados...",
          "Cleaning selected targets...", lang)
    }

    pub fn tui_finished_status(lang: Language) -> &'static str {
        s("Limpeza concluída: {done} alvos, {reclaimed} recuperados, {errors} erros.",
          "Cleaning finished: {done} targets, {reclaimed} reclaimed, {errors} errors.", lang)
    }

    pub fn tui_cancel_hint(lang: Language) -> &'static str {
        s("Pressione q para cancelar o scan",
          "Press q to cancel scan", lang)
    }

    pub fn tui_rescan_hint(lang: Language) -> &'static str {
        s("Pressione r para re-escanear",
          "Press r to rescan", lang)
    }

    pub fn tui_cancelled_status(lang: Language) -> &'static str {
        s("Scan cancelado pelo usuário.",
          "Scan cancelled by user.", lang)
    }

    pub fn scanning_progress(lang: Language) -> &'static str {
        s("Escaneando {n}/{total}",
          "Scanning {n}/{total}", lang)
    }

    pub fn scan_done_progress(lang: Language) -> &'static str {
        s("Scan concluído: {size}",
          "Scan done: {size}", lang)
    }

    pub fn cleaning_progress(_lang: Language) -> &'static str {
        "Cleaning selected targets"
    }

    pub fn cleaning_finished_progress(lang: Language) -> &'static str {
        s("Limpeza concluída",
          "Cleaning finished", lang)
    }

    pub fn target_added(lang: Language) -> &'static str {
        s("Alvo '{name}' adicionado.",
          "Target '{name}' added.", lang)
    }

    pub fn target_add_duplicate(lang: Language) -> &'static str {
        s("Alvo '{name}' já existe. Use 'acari target remove' primeiro.",
          "Target '{name}' already exists. Use 'acari target remove' first.", lang)
    }

    pub fn target_removed(lang: Language) -> &'static str {
        s("Alvo '{name}' removido.",
          "Target '{name}' removed.", lang)
    }

    pub fn target_not_found(lang: Language) -> &'static str {
        s("Alvo '{name}' não encontrado na configuração.",
          "Target '{name}' not found in config.", lang)
    }

    pub fn target_list_header(lang: Language) -> &'static str {
        s("Alvos personalizados:",
          "Custom targets:", lang)
    }

    pub fn target_list_empty(lang: Language) -> &'static str {
        s("Nenhum alvo personalizado configurado. Use 'acari target add' para adicionar um.",
          "No custom targets configured. Use 'acari target add' to add one.", lang)
    }

    pub fn target_list_builtin(lang: Language) -> &'static str {
        s("(embutido)", "(built-in)", lang)
    }

    pub fn target_list_custom(lang: Language) -> &'static str {
        s("(personalizado)", "(custom)", lang)
    }

    pub fn config_last_modified(lang: Language) -> &'static str {
        s("Config modificado pela última vez em: {time}",
          "Config last modified: {time}", lang)
    }

    pub fn config_updated_at(lang: Language) -> &'static str {
        s("Config atualizado em: {time}",
          "Config updated at: {time}", lang)
    }

    pub fn custom_targets_loaded(lang: Language) -> &'static str {
        s("{n} alvo(s) personalizado(s) carregados do config (modificado em: {time})",
          "{n} custom target(s) loaded from config (modified: {time})", lang)
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
        s("Pressione y para confirmar ou n para cancelar", "Press y to confirm or n to cancel", lang)
    }

    pub fn confirm_clean(lang: Language) -> &'static str {
        s("Confirmar limpeza de {n} alvo(s) (total: {size})? [y/n]", "Confirm clean {n} target(s) (total: {size})? [y/n]", lang)
    }

    pub fn panel_status(_lang: Language) -> &'static str {
        "Status"
    }

    pub fn panel_progress(_lang: Language) -> &'static str {
        "Progress"
    }

    pub fn panel_targets(lang: Language) -> &'static str {
        s("Alvos (espaço: marcar, a: todos, s: ordenar, enter: limpar)", "Targets (space: toggle, a: all, s: sort, enter: clean)", lang)
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
}
