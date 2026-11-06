# Desktop GPUI migration: behavior to preserve

The existing implementation in `src/ui/desktop.rs` remains the active desktop
interface until its replacement has been verified. A successful GPUI build or
component render alone does not establish parity. An opt-in read-only slice is
available via `cargo run --features gpui-kit --example acari-gpui-preview -- <path>`.
It displays progress, cancellation, errors and paginated children, with read-only
navigation and apparent/allocated sizes;
it does not offer cleanup, chart navigation or configuration. The path defaults
to the user's home directory, but no scan starts until the button is pressed.

| Journey | Existing contract | Replacement status |
| --- | --- | --- |
| Disk analysis | User selects a path or mounted volume; scanning is read-only, reports progress and errors, and can be cancelled. | Partial: path is supplied at launch; native rendering and interaction not yet verified |
| Explore results | Navigate breadcrumbs and chart views, select an item, inspect apparent or allocated sizes, filter and sort. | Partial: tree navigation and pagination implemented; charts, filtering, sorting and visual validation remain pending |
| Project roots | Roots are opt-in, validated and deduplicated; removing one edits configuration only. Inspecting a root starts read-only analysis. | Not implemented |
| Cleanup preview | Scan targets with progress and cancellation; incomplete, expired or unsupported targets cannot be selected for cleanup. | Not implemented |
| Cleanup confirmation | Snapshot target identity and dry-run mode; revalidate before execution, reject overlapping paths and require individual confirmation for special targets. | Not implemented |
| Cleanup execution | Preserve dry-run semantics, system authorization for supported privileged actions, cancellation, detailed errors, and accurate unmeasured/partial results. Invalidate previews after real cleanup or worker disconnection. | Not implemented |

Before switching the active desktop entry point, exercise these journeys with
synthetic fixtures, including failures and cancellation. Test native dialogs,
keyboard interaction and actual window rendering separately from component
tests, and check supported OS builds. Do not start real cleanup or use user data
as a visual test fixture. The user reported a bug but has not yet supplied its
reproduction steps; it cannot be treated as fixed by this migration.
