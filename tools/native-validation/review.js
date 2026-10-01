"use strict";
// Reviewed source dispositions. Intervals select individual AST sites; generated
// evidence retains each site's complete expression, location and source hash.
// A shared rationale describes a common validator, not claimed branch coverage.
const r = (from,to,reason,rust,tests,disposition="ported") => ({lines:[from,to],disposition,reason,rust,tests});
const negative = "tests-rust/profile.rs#fn captured_legacy_negative_corpus_still_fails_closed";
const models = "tests-rust/profile.rs#fn complete_legacy_models_fingerprints_bounds_and_ordered_uses_match";
const native = "tests-rust/compiled.rs#fn all_native_engine_and_synthetic_profiles_prepare_without_losing_analytic_geometry";
const plans = "tests-rust/plan.rs#fn mismatched_units_identity_machine_unknown_fields_and_unsafe_boundaries_fail";
const planIssues = "tests-rust/plan.rs#fn all_plan_failures_are_collected_and_dependent_boundaries_marked_unchecked";
const mutations = "tests-rust/compiled.rs#fn independent_audit_rejects_each_missing_or_replaced_command_and_changed_source_use";
const ordered = "tests-rust/compiled.rs#fn complete_ordered_motion_events_and_transitions_match_frozen_legacy_oracle";
const wire = "src-rust/bundle.rs#fn serialization_faults_are_rejected_by_semantics_not_just_checksums";
const wireBad = "src-rust/bundle.rs#fn malformed_truncated_stale_and_resource_exhausting_bundles_never_load";
const numeric = "tests-rust/compiled.rs#fn typed_binary_replaces_text_limits_but_refuses_nonfinite_or_zero_converted_feed";
const metadata = "tests-rust/compiled.rs#fn source_names_are_opaque_metadata_and_cannot_become_native_commands";
const parseBad = "tests-rust/part21.rs#fn rejects_dangling_duplicate_nonfinite_escape_and_incomplete_inputs";
const syntax = "tests-rust/part21.rs#fn legacy_corpus_shape_counts_and_source_hashes_match_pinned_reader";
const diag = "tests-rust/diagnostics.rs#fn stages_and_archive_summaries_do_not_claim_unreached_or_missing_checks_passed";
const graph = "tests-rust/graph.rs#fn every_ordered_use_and_shared_state_must_be_consumed";
const graphBad = "tests-rust/graph.rs#fn unknown_properties_and_bad_sequence_cannot_hide_in_graph";
const audit = "src-rust/command_audit.rs#pub fn audit";
const motion = "src-rust/command_audit.rs#fn motion(";
const phase = "src-rust/command_audit.rs#fn phase(";
const use = "src-rust/command_audit.rs#fn next(";
const process = "src-rust/command_audit.rs#fn process(";
const waypoints = "src-rust/command_audit.rs#fn waypoints(";
const state = "src-rust/profile.rs#fn state(";
const shape = "src-rust/shape.rs#pub fn validate";
const groupGraph = "src-rust/profile_graph.rs#pub fn new";

module.exports = {
  legacyRevision: "a849d0565a9f493128da015e63fcfe9b003acba6",
  otherFiles: {
    "src/configuration.js": "Diagnostics directory convention is retained. Global execution-plan and tool-table environment fallbacks are intentionally removed from native CLI; explicit inputs prevent silently selecting another setup. Directory and env-clear CLI tests verify this.",
    "src/diagnostics.js": "Replaced by native JSON/TXT archive with its own versioned schema; diagnostics.rs and native_cli.rs test success, failures, hashes, busy indexes and source collisions.",
    "src/doctor.js": "Legacy installation/environment report, not job semantic validation. Native CLI reports missing inputs/manifests and no runtime binding; it does not claim to diagnose an installed controller.",
    "src/files.js": "Exclusive G-code writes become audited bundle staging/publication and source/store collision guards in publication.rs, including real crash and injected storage tests.",
    "src/job-requirements.js": "Derived operational requirements are represented by profile::requirements and command_audit required_capabilities, checked by capabilities.rs. G-code line ranges and GUI presentation are replaced by native spans/source-use provenance.",
    "src/process-summary.js": "Derived report summaries and G-code line metrics, not admission checks. Native command audit reports ordered source uses/events and spans. Real timing/resource measurements remain the separate Stage 2 benchmark requirement.",
    "src/profile-contract.js": "Every entity arity, attribute constraint and complex combination is enumerated separately and compared exactly with src-rust/profile-shape.json.",
    "src/report.js": "Legacy HTML review and report serialization, outside the requested native compiler with no GUI/preview. Native diagnostic/archive and artifact checks do not consume HTML.",
    "src/review-client.js": "Browser interactions, outside the requested native compiler with no GUI/preview; cannot admit native commands.",
    "src/review-geometry.js": "Review-only projected geometry, travel summaries and ideal feed-time estimates. Native geometry.rs independently checks continuous curves; performance prediction/benchmarks remain separate Stage 2 work.",
    "src/review-viewer.js": "HTML viewer assembly, outside the requested native compiler with no GUI/preview; no native admission path.",
    "src/stock-preview.js": "Approximate visual stock removal, outside the requested native compiler with no GUI/preview; never established physical clearance or admission.",
    "src/translate.js": "Emitter orchestrates validators inventoried individually here. Its only throw rethrows a validator result. compiled.rs emits typed commands; command_audit.rs independently checks every ordered use, event and policy transition against decoded source/setup.",
    "src/validation-coverage.js": "Diagnostic bookkeeping, replaced by actual CLI-stage outcomes and explicit not_checked live state. Full EXPRESS/WHERE/global/AP238 checks remain unimplemented and are not implied by subset validation.",
    "vendor/fusion360next-nc/next-nc.js": "Legacy source writer used only by tests/oracles, not imported by the translator's reader. Producer qualification is Stage 1 in Fusion360Next-NC; this inventory covers consumer checks.",
    "vendor/fusion360next-nc/validation-error.js": "Exception container becomes Diagnostic/Result with structured stage, code, source and context; error preservation is covered by diagnostic and CLI tests."
  },
  rules: {
    "vendor/fusion360next-nc/part21.js": [
      r(8,8,"X2 length check retained; Rust also rejects malformed hex, invalid UTF-16 and unsupported escapes, with byte locations.",["src-rust/part21.rs#fn string("],[parseBad],"intentionally-changed"),
      r(13,13,"Default 32 MiB source bound retained; configured additional resource bounds are enforced before expansion.",["src-rust/part21.rs#pub fn parse","src-rust/part21.rs#impl Default for Limits"],["tests-rust/part21.rs#fn resource_bounds_fail_with_diagnostics_before_unbounded_expansion"]),
      r(24,24,"Same Part 21 envelope and end tokens required; Rust accepts inter-token whitespace rather than fixed newline spelling.",["src-rust/part21.rs#pub fn parse","src-rust/part21.rs#fn take("],[parseBad,"tests-rust/part21.rs#fn reads_complex_entities_unicode_and_exact_input_identity"],"intentionally-changed"),
      r(33,35,"Typed parse errors carry source byte locations; required tokens and missing tokens fail.",["src-rust/part21.rs#fn error(","src-rust/part21.rs#fn take("],[parseBad]),
      r(37,37,"Depth remains bounded; default 64 and an explicitly checked maximum configuration replace unbounded recursion.",["src-rust/part21.rs#fn value(","src-rust/part21.rs#pub fn check"],["tests-rust/part21.rs#fn resource_bounds_fail_with_diagnostics_before_unbounded_expansion"]),
      r(42,42,"Entity names use the supported uppercase identifier grammar.",["src-rust/part21.rs#fn ident("],[parseBad,syntax]),
      r(48,48,"References must be positive safe integer identities.",["src-rust/part21.rs#fn id("],[parseBad]),
      r(52,53,"Terminated strings and recognized escapes are required; errors remain failures with source context.",["src-rust/part21.rs#fn string("],[parseBad]),
      r(58,58,"All parsed numbers must be finite and lexically valid.",["src-rust/part21.rs#fn value("],[parseBad]),
      r(63,91,"Header order/attributes, positive unique record IDs, nonempty unique components and all reference targets are checked over the complete document.",["src-rust/part21.rs#pub fn parse","src-rust/part21.rs#fn id("],[parseBad,syntax])
    ],
    "vendor/fusion360next-nc/inspect.js": [
      r(17,17,"Inspection failures become structured Rust errors, not generic legacy wrappers.",["src-rust/diagnostic.rs#pub struct Diagnostic"],[negative,diag],"intentionally-changed"),
      r(18,18,"Positive process/radius/dwell measures remain finite and strictly positive.",["src-rust/profile.rs#fn positive("],[models,negative]),
      r(19,19,"Closed INTEGRATED_CNC_SCHEMA only.",[shape],[syntax,negative]),
      r(33,45,"Source record errors and valid unique operation/tool/relationship associations are retained.",[groupGraph,"src-rust/profile_graph.rs#pub fn fail"],[graph,negative]),
      r(53,56,"Single referenced component and singleton collections remain mandatory.",["src-rust/part21.rs#pub fn entity","src-rust/profile.rs#fn one("],[negative,models]),
      r(59,70,"Duplicate, unrepresented or missing properties fail; required representation types are checked.",[groupGraph,"src-rust/profile_graph.rs#pub fn property"],[graphBad,negative]),
      r(92,96,"Ordered uses remain unique; sequences must be nonempty, contiguous and one-based.",["src-rust/profile_graph.rs#pub fn visit","src-rust/profile_graph.rs#pub fn sequence"],[graph,graphBad]),
      r(102,132,"Cycle rejection and exact supported unit names/factors/dimensions/numerator/denominator are retained, with an added graph-depth bound.",["src-rust/units.rs#pub fn resolve","src-rust/units.rs#fn resolve_inner"],["tests-rust/units.rs#fn unit_cycle_and_wrong_conversion_are_explicit_errors","tests-rust/units.rs#fn legacy_dimensions_resolve_without_nominal_spindle_speed_conversions"]),
      r(135,137,"One three-dimensional context with exactly length/radian/steradian units remains required.",["src-rust/units.rs#pub fn geometry_context"],[models,negative]),
      r(139,139,"Geometry must use the selected context and one geometry item.",["src-rust/profile.rs#fn geometry_item("],[models,negative]),
      r(142,143,"Measure type, finite scalar, cardinality and dimension must match the use.",["src-rust/units.rs#pub fn measure"],[models,negative]),
      r(148,171,"Technology/functions type, feed reference, RPM/CSS restrictions, measure counts and explicit supported feeds/coolant are retained.",[state],[models,negative]),
      r(179,179,"Points require three finite coordinates; lathe Y must be zero.",["src-rust/profile.rs#fn point(","src-rust/profile.rs#fn three("],[models,negative]),
      r(183,183,"Finite unit direction uses the same 1e-9 norm threshold.",["src-rust/profile.rs#fn vector("],[models,negative]),
      r(188,190,"Revision-1 profile and coordinate conventions retained; version 2 explicitly adds tolerance, movement, requirements and analytic circular geometry.",[groupGraph,"src-rust/profile.rs#pub fn decode_document"],[models,"tests-rust/profile.rs#fn native_extensions_are_closed_and_capabilities_are_checked_for_the_whole_job"],"intentionally-changed"),
      r(206,228,"All legacy plane/sense/trim/full-circle/radius/reference-direction checks remain in the revision-1 arc reader. Revision-2 helices use a distinct explicit representation and independent geometry audit.",["src-rust/profile.rs#fn arc(","src-rust/geometry.rs#pub fn validate_with_floor"],[models,negative,native]),
      r(239,239,"Decimal tool/offset/WCS labels require safe nonnegative/positive integers with the original minimum.",["src-rust/profile.rs#fn nonnegative("],[models,negative]),
      r(251,253,"Workingstep/operation names agree and resources must be cutting tools.",["src-rust/profile.rs#fn section("],[models,negative]),
      r(266,299,"Required priority, trajectory direction/type, rapid semantics, polyline cardinality, no rapid arcs, feed presence and 1e-9 source continuity are retained.",["src-rust/profile.rs#fn path("],[models,negative,native]),
      r(313,315,"All executable uses, shared process/tool definitions and audited relationships must be consumed; orphan suffixes fail the complete job.",["src-rust/profile_graph.rs#pub fn finish"],[graph,negative])
    ],
    "src/profile.js": [
      r(31,34,"Bounded source and the closed schema are required before semantic decoding.",["src-rust/part21.rs#pub fn parse",shape],[syntax,negative]),
      r(37,39,"Property associations must reference single entities with empty unsupported semantic fields.",["src-rust/profile_graph.rs#pub fn single",groupGraph],[negative]),
      r(45,45,"Closed per-owner property names retained for v1; v2 adds only explicitly named native fields.",[groupGraph],[graphBad,"tests-rust/profile.rs#fn native_extensions_are_closed_and_capabilities_are_checked_for_the_whole_job"],"intentionally-changed"),
      r(48,77,"Property representations/labels, relationship endpoints, methods and ambiguous dwell/coolant/unused feed fields remain checked.",[groupGraph],[graphBad,negative]),
      r(87,90,"Through-tool coolant is rejected without substitution, and dwell cannot carry cutting feed.",[state,"src-rust/profile.rs#fn path("],[negative,"tests-rust/profile.rs#fn native_extensions_are_closed_and_capabilities_are_checked_for_the_whole_job"]),
      r(97,97,"Original typed error is preserved with stage/context and archived by the native CLI.",["src-rust/main.rs#fn main()","src-rust/diagnostics.rs#pub fn archive"],[diag],"intentionally-changed")
    ],
    "src/profile-shape.js": [r(37,61,"Same embedded grammar, component combinations, attribute arity and recursive shape constraints, including SI-dependent dimension spelling. Unknown grammar kinds reject rather than throw an unstructured JS exception.",[shape,"src-rust/shape.rs#fn matches"],[syntax,negative,"tests-rust/part21.rs#fn every_legacy_entity_arity_and_attribute_rejects_an_invalid_value"])],
    "src/continuity.js": [
      r(12,12,"The first operation cannot use continue/link; it needs a reviewed retract/approach.",["src-rust/plan.rs#pub fn validate"],[plans,negative]),
      r(13,21,"Exact source and mapped tool/offset/WCS, spindle mode/speed/direction/cap and coolant compatibility remain required.",["src-rust/plan.rs#fn compatible("],[plans,negative,ordered]),
      r(29,29,"Continuation requires exact commanded exit/entry with no invented link, including full-circle exit semantics.",["src-rust/plan.rs#pub fn exit_point","src-rust/plan.rs#pub fn validate"],[plans,ordered])
    ],
    "src/plan.js": [
      r(13,13,"Closed object keys remain validated, including per-transition-mode allowed fields.",["src-rust/plan.rs#fn keys("],[plans,negative]),
      r(16,25,"Reviewed paths retain waypoint counts, single allowed axis, finite magnitude below 1e9, complete axes and exact order.",["src-rust/plan.rs#fn waypoints("],[plans,negative,ordered]),
      r(31,37,"Schemas 1-4 retain exact-path defaults and original binding rules. Native schema 5 adds explicit per-operation path control with separately reviewed additional blend allowance; machine/unit/fingerprint mappings and one transition per section remain mandatory.",["src-rust/plan.rs#pub fn validate","src-rust/plan.rs#fn path_control("],[plans,negative,"tests-rust/path_control.rs#fn missing_unknown_or_invalid_additional_allowances_fail_the_complete_plan"],"intentionally-changed"),
      r(42,42,"Rust Result replaces JS exception type discrimination; collected plan issues and dependent unchecked boundaries remain visible.",["src-rust/plan.rs#pub fn validate"],[planIssues],"intentionally-changed"),
      r(51,56,"Explicit T/H mappings retain positive integer 1..99999 bounds and WCS mapping retains G54..G59.3.",["src-rust/plan.rs#fn integer(","src-rust/plan.rs#pub fn validate"],[plans,negative]),
      r(67,89,"Schema-dependent transition modes, compatible continue/link state and exact reviewed link/approach endpoints remain required.",["src-rust/plan.rs#pub fn validate","src-rust/plan.rs#fn compatible("],[plans,negative,ordered]),
      r(97,97,"All independent plan issues are collected; dependent boundary checks are identified as unchecked, not passed.",["src-rust/plan.rs#pub fn validate"],[planIssues])
    ],
    "src/tool-table.js": [r(11,40,"Complete optional table snapshot retains 1 MiB bound, recognized words, duplicate/nonfinite rejection, exact T/P/Q integer spelling/ranges, required T/P, unique T and all mapped T/H record presence. Missing snapshot is explicitly not_checked.",["src-rust/tool_table.rs#pub fn check"],["tests-rust/plan.rs#fn tool_table_rejects_bad_unused_records_and_reports_all_missing_uses",negative])],
    "src/command-contract.js": [
      r(14,21,"Typed variants plus independent complete phase/use audit replace arbitrary object fields and enforce initialization, order and a single terminal end. Binary unknown tags/trailing commands fail.",[use,phase,audit,"src-rust/bundle.rs#fn decode("],[mutations,wireBad],"intentionally-changed"),
      r(23,23,"Names are opaque source metadata; native command vocabulary has no executable comment/string variant.",["src-rust/compiled.rs#pub enum Action","src-rust/bundle.rs#fn decode("],[metadata],"intentionally-changed"),
      r(25,27,"Canonical mm and typed per-motion plane replace mutable G20/G21/G17/G18/G19 state; source units and lathe planes remain validated.",[motion,"src-rust/compiled.rs#pub(crate) fn scale"],[ordered,native],"intentionally-changed"),
      r(29,30,"Typed atomic spindle variants retain RPM/CSS, positive finite demand, CSS cap and mill restrictions. Decimal magnitude ceiling is replaced by finite binary64 checks; it was a G-code encoding bound, not a machine limit.",[process,"crates/motion-command/src/lib.rs#pub fn check_structure"],[ordered,numeric],"intentionally-changed"),
      r(32,33,"Direction is a typed Boolean within an atomic speed/mode event; no speedless spindle-start variant exists. Independent source event audit preserves reversals.",[process,"crates/motion-command/src/lib.rs#pub enum Spindle"],[ordered,mutations],"intentionally-changed"),
      r(34,34,"Only supported Off/Flood/Mist coolant variants and ordered source changes are allowed.",[process],[ordered,mutations]),
      r(36,36,"Dimensioned finite positive feed is carried on motion; restoration is an explicit end intent. Binary64 replaces decimal magnitude/precision bounds and rejects zero/nonfinite conversion results.",[motion,"src-rust/compiled.rs#fn feed("],[ordered,numeric],"intentionally-changed"),
      r(39,41,"Tool/H and WCS commands must exactly match the bounded reviewed mappings at their required positions.",[audit,"src-rust/plan.rs#fn integer("],[ordered,mutations]),
      r(43,44,"Dwell remains positive/finite and ordered at source position under established process state. Typed work-coordinate plan replaces modal plane/units and decimal magnitude checks.",[audit,process,"crates/motion-command/src/lib.rs#pub fn check_structure"],[ordered,mutations,numeric],"intentionally-changed"),
      r(48,57,"Typed work-frame XYZ/XZ motion, plane/sense/center/endpoint and reviewed machine-frame rapid intents replace modal/partial-axis text. Legacy arc reconstruction is preserved; v2 adds explicit sweep/rise. Exact source and continuous geometry audits reject incompatible fields. Finite binary64 replaces decimal number bounds.",[motion,waypoints,"src-rust/geometry.rs#pub fn validate_with_floor"],[ordered,mutations,native,numeric],"intentionally-changed")
    ],
    "src/execution-audit.js": [
      r(49,61,"Compact spans and command records replace G-code/source-map line pairs; independent traversal requires every ordered phase/use and no extras.",[phase,use,audit],[ordered,mutations],"intentionally-changed"),
      r(71,71,"Exact reviewed waypoint, frame, axis, phase and ordinal are independently checked.",[waypoints],[ordered,mutations]),
      r(81,86,"Source metadata is interned once and resolved from audited span/use identities, not duplicated per-command snapshots. Mapping/process obligations are folded independently.",[use,process,"src-rust/compiled.rs#pub(crate) fn source_context"],[ordered,mutations],"intentionally-changed"),
      r(94,113,"Every dwell/linear/arc use preserves source order, source vertices, geometry/feed/frame and relative-center behavior. Zero-length/backtracking uses remain present.",[motion,audit],[ordered,mutations,"tests-rust/compiled.rs#fn repeated_and_backtracking_vertices_remain_distinct_ordered_uses"]),
      r(123,142,"Command/span identity, typed action kind, complete phase coverage, terminal end and operation ranges replace line-number metadata. Each source use is still required and independently audited.",[phase,use,audit],[ordered,mutations],"intentionally-changed")
    ],
    "src/policy-audit.js": [
      r(22,38,"Independent Rust audit folds source process demand and exact required commands; no emitter state snapshot is trusted.",[process,audit],[ordered,mutations]),
      r(48,62,"Startup reset/stop, compatible transitions, all reviewed waypoints, necessary tool change and initial process state are required before path entry.",[audit,phase,waypoints,process],[ordered,mutations]),
      r(78,79,"Every source path has the required process state and typed plane; there is no inherited arc-plane mode in native motion.",[process,motion],[ordered,mutations,native],"intentionally-changed"),
      r(85,116,"Stopped retract/approach/end paths, unchanged link state, mapping-after-retract, tool-change reset and exact tool/WCS/offset/reset ordering are independently required.",[audit,waypoints,process],[ordered,mutations]),
      r(127,129,"Direction reversal retains an explicit stop; atomic typed spindle events prevent speedless or contradictory split starts.",[process],[ordered,mutations],"intentionally-changed"),
      r(134,150,"End retract precedes offset cancellation/feed restoration; stopped/off completion is terminal and unique. Link state cannot change temporarily and recover later.",[audit,process],[ordered,mutations])
    ],
    "src/gcode-audit.js": [
      r(7,15,"Binary candidate is independently decoded and audited; length/count/tag/span coverage replace text newline/block/line association limits. Partial, trailing or corrupt objects cannot load.",["src-rust/bundle.rs#fn decode(",audit],[wireBad,wire],"intentionally-changed"),
      r(19,28,"No native executable comment language exists. Source labels remain opaque UTF-8 metadata and cannot create commands; TXT diagnostics escape controls separately.",["src-rust/compiled.rs#pub enum Action","src-rust/diagnostics.rs#fn visible"],[metadata,"tests-rust/diagnostics.rs#fn utc_and_readable_control_escaping_cover_calendar_boundaries"],"intentionally-changed"),
      r(35,82,"Explicit binary tags/scalars and independent source/geometry/policy audits replace G-code word and modal-state decoding. Exact complete coverage includes reset-after-tool-change, supported axes/planes and one final end; malformed/nonfinite/trailing records fail.",["src-rust/bundle.rs#fn decode(",motion,audit],[wireBad,wire,mutations],"intentionally-changed")
    ],
    "src/linuxcnc-output.js": [
      r(6,12,"Decimal magnitude/exponent-expansion limits are text-encoding restrictions. Native finite binary64 is bounded in bytes and independently decoded; converted nonfinite/zero cutting feed fails. This does not establish physical machine limits.",["src-rust/bundle.rs#fn f64(",motion],[numeric],"intentionally-changed"),
      r(23,23,"Machine/units come from the validated closed source profile; typed geometry is checked for that machine.",["src-rust/profile.rs#pub fn decode_document",motion],[models,native]),
      r(71,73,"Exhaustive Rust enum encoding and unknown-tag rejection replace string command dispatch and 240-character G-code lines.",["src-rust/bundle.rs#fn record(","src-rust/bundle.rs#fn decode("],[wireBad,wire],"intentionally-changed")
    ],
    "src/errors.js": [r(5,5,"Rust Result/Diagnostic preserves failure, context and correction guidance without JS exception wrapping.",["src-rust/diagnostic.rs#pub struct Diagnostic","src-rust/diagnostics.rs#pub fn correction"],[diag,negative],"intentionally-changed")],
    "src/internal-error.js": [r(9,9,"Native invariant diagnostics remain distinct from source/plan validation and cannot authorize a partial plan.",["src-rust/command_audit.rs#fn fail(","src-rust/compiled.rs#pub fn prepare"],[mutations,wire],"intentionally-changed")],
    "src/artifact-identity.js": [
      r(6,6,"Native bundle commands require an explicit artifact; optional source preflight does not claim artifact verification. There is no implicit G-code/report pairing.",["src-rust/main.rs#fn run("],["tests-rust/native_cli.rs#fn standalone_publication_and_bundle_verification_need_no_node_or_live_state"],"intentionally-changed"),
      r(13,23,"Regular descriptor, nonblocking POSIX open, before/after metadata and actual bounded byte count remain checked. Native bundle capacity is explicit 256 MiB, separately from source capacity; publication additionally rereads snapshots and validates content identity.",["src-rust/fileio.rs#pub(crate) fn read_bytes","src-rust/publication.rs#fn snapshot("],["tests-rust/native_cli.rs#fn native_input_rejects_fifo_without_waiting_for_a_writer","src-rust/publication.rs#fn changed_source_setup_table_and_corrupt_cache_disarm_before_use",wireBad],"intentionally-changed"),
      r(25,29,"A stale optional report hash cannot authorize native reuse. Bundle identities and content-addressed object hashes are checked with fresh selection generations; explicit verification returns exact observed identity without claiming authenticity/live loading.",["src-rust/bundle.rs#fn decode(","src-rust/publication.rs#fn read_artifact("],[wireBad,"src-rust/publication.rs#fn published_job_is_reusable_without_old_project_files_but_never_auto_selected"],"intentionally-changed")
    ],
    "bin/nextnc.js": [
      r(16,16,"Native reads require a regular file, bounded actual bytes, valid UTF-8 and stable descriptor metadata. Source/setup share the explicit native input budget; table/target remain 1 MiB. POSIX FIFO reads cannot block awaiting a writer.",["src-rust/fileio.rs#pub(crate) fn read_bytes","src-rust/diagnostics.rs#pub fn read_text","src-rust/main.rs#const OPTION_LIMIT"],["tests-rust/native_cli.rs#fn native_input_rejects_fifo_without_waiting_for_a_writer","tests-rust/diagnostics.rs#fn read_bounds_invalid_utf8_and_excerpt_bounds_preserve_honest_identity"],"intentionally-changed"),
      r(24,100,"Native CLI has explicit source/setup/table/target/store inputs and its own closed command syntax; no global plan fallback or GUI/G-code filter admission. Strict JSON rejects duplicates and reports the actual input. All errors exit without success stdout and retain diagnostic evidence. Legacy report/doctor/template/filter commands remain in the existing executable, outside the native compiler.",["src-rust/main.rs#fn run(","src-rust/main.rs#fn option_paths(","src-rust/json.rs#pub fn parse"],["tests-rust/native_cli.rs#fn standalone_cli_has_no_node_path_and_publishes_only_complete_preflight_json","tests-rust/native_cli.rs#fn persistent_cli_reports_cover_all_commands_and_preserve_failure_when_archiving_fails","tests-rust/json.rs#fn strict_json_rejects_duplicates_at_every_depth_and_resource_limit"],"intentionally-changed")
    ]
  }
};
