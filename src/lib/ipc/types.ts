// Every DTO in docs/contracts/ipc.md §2–§6, in TypeScript (ADR-004). Types that ts-rs already
// generates are re-exported from `bindings/` so there is one definition; the rest are the mechanical
// camelCase translation of the Rust in ipc.md and move to `bindings/` as foreman-app exports them.
//
// Contract gaps (types ipc.md names but never defines) are marked `GAP:` with the shape the UI
// needs; each is listed in docs/dev-notes.md for M2-01 to settle in the contract.
import type { AppBootstrap as AppBootstrapBinding } from './bindings/AppBootstrap.ts';
import type { ClipboardProblem } from './bindings/ClipboardProblem.ts';
import type { DoctorLevel } from './bindings/DoctorLevel.ts';
import type { IpcError } from './bindings/IpcError.ts';
import type { IpcErrorCode } from './bindings/IpcErrorCode.ts';
import type { IpcErrorDetail } from './bindings/IpcErrorDetail.ts';
import type { ProjectSummary } from './bindings/ProjectSummary.ts';
import type { ReadOnly } from './bindings/ReadOnly.ts';
import type { ReadOnlyReason } from './bindings/ReadOnlyReason.ts';
import type { StaleWhat } from './bindings/StaleWhat.ts';
import type { ToolKind } from './bindings/ToolKind.ts';

export type {
	ClipboardProblem,
	DoctorLevel,
	IpcError,
	IpcErrorCode,
	IpcErrorDetail,
	ProjectSummary,
	ReadOnly,
	ReadOnlyReason,
	StaleWhat,
	ToolKind
};

// §2.1 scalars
export type ProjectId = string;
export type RunId = string;
export type ChainId = string;
export type PlanId = string;
export type PreviewId = string;
export type ChangeId = string;
export type PickId = string;
export type HitId = string;
export type StepKey = string;
export type ItemId = string;
export type Stage = number;
export type Rev = number;
export type RepoPath = string;
export type Sha256 = string;
export type Timestamp = string;
export type IsoDate = string;
export type EnvName = string;
export type BundleId = string;
/** `Record<symbol, never>`: the request of a command with no fields. */
export type Empty = Record<symbol, never>;

// §2.2 enums
export type TriState = 'yes' | 'no' | 'undecided';
export type ConditionId =
	| 'idea_known'
	| 'commercial'
	| 'payments'
	| 'multi_user'
	| 'public'
	| 'has_ui'
	| 'others_data'
	| 'server'
	| 'has_database'
	| 'has_uploads'
	| 'has_email'
	| 'has_integrations';
export type Surface = 'chat' | 'cli' | 'desktop';
export type CwdKind = 'project' | 'spec';
export type Model = 'fable' | 'opus' | 'sonnet' | 'haiku' | 'opusplan';
export type Effort = 'low' | 'medium' | 'high' | 'xhigh' | 'max' | 'ultracode' | 'adaptive';
export type Mode = 'plan' | 'default' | 'acceptEdits' | 'auto' | 'dontAsk';
export type EntryPoint = 'doNext' | 'palette' | 'stageView';
export type StepState =
	| 'notStarted'
	| 'copied'
	| 'running'
	| 'done'
	| 'skipped'
	| 'notApplicable'
	| 'waitingUntil'
	| 'undecided'
	| 'needsAnnotation'
	| 'needsNewerForeman';
export type RunStatus =
	'starting' | 'running' | 'idle' | 'blocked' | 'exited' | 'errored' | 'interrupted';
export type PasteKind = 'auto' | 'packet' | 'output' | 'adrSet' | 'lessons';
export type CopyTarget = 'prompt' | 'command' | 'sessionId' | 'fileContents' | 'fixRequest';
export type Theme = 'system' | 'vellum' | 'night';

// §2.3 structured types
export interface Start {
	model: Model;
	effort: Effort;
	mode: Mode | null;
}
export type ValueOrigin =
	| { kind: 'template' }
	| { kind: 'alternative'; rule: string }
	| { kind: 'substitution'; rule: string }
	| { kind: 'override' };
export interface Explained<T> {
	value: T;
	origin: ValueOrigin;
	from: T | null;
}
export interface RunOverride {
	model: Model | null;
	effort: Effort | null;
	mode: Mode | null;
}

export type TextSpan =
	| { kind: 'plain'; text: string }
	| { kind: 'escape'; codepoint: number; label: string }
	| { kind: 'code'; text: string }
	| { kind: 'strong'; children: TextSpan[] }
	| { kind: 'emph'; children: TextSpan[] }
	| { kind: 'link'; text: string; url: string };
export type DocNode =
	| { kind: 'heading'; level: number; spans: TextSpan[] }
	| { kind: 'paragraph'; spans: TextSpan[] }
	| { kind: 'list'; ordered: boolean; items: DocNode[][] }
	| { kind: 'quote'; children: DocNode[] }
	| { kind: 'codeBlock'; info: string; text: string }
	| { kind: 'rule' };

export type TokenClass =
	'projectVariable' | 'packetMarker' | 'pathAlias' | 'runInput' | 'optionalPart' | 'literal';
export type PromptSegment =
	| { kind: 'text'; spans: TextSpan[] }
	| { kind: 'resolved'; token: string; class: TokenClass; text: string; note: string | null }
	| { kind: 'unresolved'; token: string; class: TokenClass; input: string | null }
	| { kind: 'optionalPart'; partId: string; included: boolean; text: string }
	| { kind: 'removedPart'; expr: string; reason: string };

export type DiffOp = 'ctx' | 'add' | 'del';
export interface DiffLine {
	op: DiffOp;
	spans: TextSpan[];
}
export interface DiffHunk {
	oldStart: number;
	newStart: number;
	lines: DiffLine[];
}
export interface Diff {
	path: RepoPath;
	exists: boolean;
	hunks: DiffHunk[];
}
export type ChangeKind = 'added' | 'removed' | 'valueChanged';
export interface RedactedEntry {
	keyPath: string;
	change: ChangeKind;
	before: string | null;
	after: string | null;
}
export interface RedactedDiff {
	source: string;
	entries: RedactedEntry[];
}

export type FixAction = 'repair' | 'openInEditor' | 'copyFixRequest';
export interface Problem {
	rule: string;
	line: number | null;
	message: string;
	fix: FixAction[] | null;
}

// Blocker and ConfirmReason (§2.3): tagged by `kind`.
export type NotRunnableReason =
	| 'notApplicable'
	| 'undecided'
	| 'needsAnnotation'
	| 'done'
	| 'needsNewerForeman'
	| 'waiting'
	| 'running';
export type ToolPathProblem =
	'relative' | 'missing' | 'notExecutable' | 'inProject' | 'symlinkIntoProject';
export type AuthProblem =
	| { kind: 'wrongMethod'; method: string }
	| { kind: 'unknownMethod' }
	| { kind: 'unparsable' }
	| { kind: 'exit'; code: number }
	| { kind: 'timeout' };
export interface Requirement {
	what: string;
	version: string;
}
export type Blocker =
	| { kind: 'notRunnable'; reason: NotRunnableReason; text: string }
	| ({ kind: 'readOnly' } & ReadOnly)
	| { kind: 'claudeMissing'; path: string | null; why: ToolPathProblem }
	| { kind: 'versionBelowFloor'; found: string; required: string; requirements: Requirement[] }
	| { kind: 'contractFailed'; feature: string; version: string }
	| { kind: 'configChanged'; cwd: CwdKind; fingerprint: Sha256; diffs: RedactedDiff[] }
	| { kind: 'authKeyBlocked'; file: string; key: string; hit: HitId; allowlistable: boolean }
	| { kind: 'configTooLarge'; file: string; bytes: number }
	| { kind: 'projectEnvUnapproved'; file: string; sha256: Sha256 }
	| { kind: 'authFailed'; why: AuthProblem }
	| { kind: 'transcriptMissing'; chain: ChainId }
	| { kind: 'signedPacketMissing'; stage: Stage }
	| { kind: 'packetUnsigned'; stage: Stage }
	| { kind: 'runInputMissing'; name: string }
	| { kind: 'unresolvedToken'; token: string }
	| { kind: 'promptOptionLike'; first: string }
	| { kind: 'promptControlChar'; offset: number; codepoint: number }
	| { kind: 'runSettingsPathUnsafe'; path: string }
	| { kind: 'cwdNotDirectory'; cwd: CwdKind };
export type ConfirmReason =
	| { kind: 'overlayStep' }
	| { kind: 'modeOrModelChanged'; from: Start; to: Start }
	| { kind: 'firstLaunchInProject' }
	| { kind: 'autoOrDontAsk'; mode: Mode }
	| { kind: 'claudeVersionChanged'; from: string; to: string };

// §4.1 App
/** GAP: `RunSummary` (ipc.md uses it in AppBootstrap and RunsPage without defining it). */
export interface RunSummary {
	runId: RunId;
	chain: ChainId;
	project: ProjectId;
	projectName: string;
	step: StepKey | null;
	stepTitle: string;
	tabName: string;
	ranWith: string;
	substituted: string | null;
	status: RunStatus;
	exitCode: number | null;
	error: string | null;
	startedAt: Timestamp;
	timeToStartMs: number | null;
	/** argv with the prompt replaced by `<prompt sha256:…>` (SR-52). */
	argv: string[];
	resumable: boolean;
	whyNotResumable: string | null;
}
/** GAP: `Capabilities` (AppBootstrap): what this Claude Code supports, as the UI needs it. */
export interface Capabilities {
	claudeVersion: string | null;
	desktop: boolean;
	sensor: boolean;
	probesVerified: boolean;
}
/** `AppBootstrap` as ts-rs generates it today (M0); `interrupted` and `capabilities` await the GAPs. */
export type AppBootstrap = AppBootstrapBinding;

/** GAP: `ToolPaths`. */
export interface ToolPathView {
	path: string | null;
	version: string | null;
	autoDetected: boolean;
	problem: string | null;
}
export interface ToolPaths {
	claude: ToolPathView;
	git: ToolPathView;
}
/** GAP: `TemplateHealthSummary`. */
export interface TemplateHealthSummary {
	runnable: string;
	needsAnnotation: string;
	errors: number;
	warnings: number;
}
export interface BlueprintRepoInfo {
	displayPath: string;
	commit: string;
	version: string;
	grammarVersion: number;
	dirty: boolean;
	health: TemplateHealthSummary;
	/** GAP: the setup form's fields come from the blueprint (variables and conditions, F-018). */
	variables: VariableSpec[];
	conditions: ConditionSpec[];
}
export interface VariableSpec {
	name: string;
	label: string;
	placeholder: string;
	multiline: boolean;
	fromSettings: boolean;
}
export interface ConditionSpec {
	id: ConditionId;
	label: string;
	effect: string;
	late: boolean;
}
export interface TerminalPrefs {
	fontSize: number;
	scrollback: number;
	macOptionIsMeta: boolean;
}
/** GAP: `LoginPathView`. */
export interface LoginPathView {
	capturedAt: Timestamp;
	entries: string[];
}
/** GAP: `AllowlistEntry`. */
export interface AllowlistEntry {
	sha256: Sha256;
	file: string;
	key: string;
	addedAt: Timestamp;
}
export interface SettingsView {
	rev: Rev;
	driName: string | null;
	tools: ToolPaths;
	editorBundleId: BundleId | null;
	blueprintRepo: BlueprintRepoInfo | null;
	theme: Theme;
	terminal: TerminalPrefs;
	envPassthrough: EnvName[];
	loginPath: LoginPathView | null;
	globalVariables: Record<string, string>;
	authKeyAllowlist: AllowlistEntry[];
	/** GAP: `screen_reader_mode` is patchable (settings_update) but missing from SettingsView. */
	screenReaderMode: boolean;
	/** GAP: Settings → Data shows the run-history size; ipc.md has no field for it. */
	runHistory: { runs: number; megabytes: number };
}
export type EditorSetting = { kind: 'set'; bundleId: string } | { kind: 'clear' };
export interface SettingsPatch {
	driName: string | null;
	editor: EditorSetting | null;
	theme: Theme | null;
	terminal: TerminalPrefs | null;
	envPassthrough: string[] | null;
	globalVariables: Record<string, string> | null;
	resetTool: ToolKind | null;
	clearLoginPath: boolean | null;
	removeAllowlistEntry: string | null;
	screenReaderMode: boolean | null;
	osDisplay: { reduceTransparency: boolean; increaseContrast: boolean } | null;
}

/** GAP: `InboxKind` (the order is AC-F017-07's). */
export type InboxKind =
	| 'sessionBlocked'
	| 'sessionYourTurn'
	| 'packetLanded'
	| 'packetInvalid'
	| 'gateFailed'
	| 'gateSignable'
	| 'outsideChange'
	| 'tamper'
	| 'chatAwaitingPaste'
	| 'waitElapsed'
	| 'interruptedSession'
	| 'stepReady';
export interface InboxItem {
	kind: InboxKind;
	project: ProjectId;
	step: StepKey | null;
	stage: Stage | null;
	at: Timestamp;
	/** GAP: Rust-side template text (SR-56), so the webview never composes it from repo content. */
	projectName: string;
	title: string;
	detail: string;
	chain: ChainId | null;
	run: RunId | null;
}

// §4.3–4.4 project and steps
/** GAP: `Tag`. `added` = a project-only step (F-025). */
export type Tag = 'core' | 'conditional' | 'optional' | 'added';
export type StepKind =
	'prompt' | 'followup' | 'action' | 'manual' | 'wait' | 'repeatable' | 'gate' | 'lessons';
/** GAP: `SpecLine`: the run config as display text, each edited value explained (F-005). */
export interface SpecLine {
	surface: Surface | 'action';
	where: string;
	model: Explained<string> | null;
	effort: Explained<string> | null;
	mode: Explained<string> | null;
	agents: string[];
	droppedAgents: string[];
	note: string | null;
}
export interface StepSummary {
	key: StepKey;
	number: string;
	title: string;
	state: StepState;
	tag: Tag;
	surface: Surface | null;
	specLine: SpecLine | null;
	whyNotRunnable: string | null;
	/** GAP: the state word with its detail ("Done, wrote 4 agents", "Running in fm-…"). */
	stateText: string;
	/** GAP: the condition behind a not-applicable or undecided row ("commercial = no"). */
	conditionNote: string | null;
	kind: StepKind;
	/** GAP: the run of a running step, to focus its tab. */
	run: RunId | null;
	/** GAP: true when Run sits behind another step still running in the same chain. */
	next: boolean;
}
/** GAP: `Applicability`. */
export type Applicability =
	| { kind: 'applies' }
	| { kind: 'notApplicable'; reason: string }
	| { kind: 'undecided'; condition: ConditionId }
	| { kind: 'needsAnnotation' };
/** GAP: a sheet's status in the strip (components.md "Sheet strip"). */
export type SheetStatus =
	| 'locked'
	| 'ready'
	| 'inProgress'
	| 'inspecting'
	| 'waiting'
	| 'signed'
	| 'notApplicable'
	| 'needsAnnotation'
	| 'undecided';
/** GAP: `SignedInfo`. */
export interface SignedInfo {
	date: IsoDate;
	commit: string;
	branch: string;
	merged: boolean;
}
/** GAP: `PacketSummary`. */
export interface PacketSummary {
	path: RepoPath;
	exists: boolean;
	problems: number;
	signed: boolean;
}
export interface StageView {
	stage: Stage;
	title: string;
	applicable: Applicability;
	signed: SignedInfo | null;
	steps: StepSummary[];
	packet: PacketSummary | null;
	/** GAP: the stage's one-line purpose shown under its title. */
	subtitle: string;
	status: SheetStatus;
	/** GAP: the spoken reason in the strip ("until Stage 2 is signed", "payments = undecided"). */
	statusWhy: string | null;
}
/** GAP: `ConditionView`. */
export interface ConditionView {
	value: TriState;
	locked: boolean;
	lockRule: string | null;
	label: string;
	effect: string;
	late: boolean;
}
export interface ProjectView {
	summary: ProjectSummary;
	conditions: Partial<Record<ConditionId, ConditionView>>;
	stages: StageView[];
	pendingChanges: number;
	readOnly: ReadOnly | null;
	/** GAP: title-block and title-bar facts (F-030). */
	branch: string;
	blueprint: { version: string; commit: string };
	dri: string | null;
	chatProjectUrl: string | null;
	/** GAP: the config-changed banner (F-008) once a fingerprint moved. */
	configChanged: ConfigChangedNotice | null;
}
/** GAP: the config-changed banner's content. */
export interface ConfigChangedNotice {
	cwd: CwdKind;
	fingerprint: Sha256;
	blocking: boolean;
	title: string;
	detail: string;
	diffs: RedactedDiff[];
}
/** GAP: `SchemaState` (setup_pick_folder). */
export type SchemaState = 'current' | 'older' | 'newer';
export interface FolderPick {
	pickId: PickId;
	displayPath: string;
	isGitRepo: boolean;
	hasForeman: SchemaState | null;
	refusal: string | null;
}
export interface PlannedWrite {
	path: RepoPath;
	action: 'create' | 'leaveAlone' | 'refuse';
	reason: string | null;
}
/** GAP: `PinPreview`. */
export interface PinPreview {
	version: string;
	commit: string;
	clean: boolean;
}
/** GAP: `ApplicablePathSummary`. */
export interface ApplicablePathRow {
	stages: string;
	title: string;
	status: 'applies' | 'notApplicable' | 'undecided' | 'needsAnnotation';
	why: string;
}
export interface ApplicablePathSummary {
	applies: number;
	total: number;
	summary: string;
	rows: ApplicablePathRow[];
	strip: { stage: Stage; status: SheetStatus; why: string | null }[];
}
export interface SetupPreview {
	previewId: PreviewId;
	writes: PlannedWrite[];
	gitInit: boolean;
	pin: PinPreview;
	path: ApplicablePathSummary;
}

/** GAP: `RunInputView` (F-006 `in=` annotations). */
export interface RunInputView {
	name: string;
	label: string;
	token: string;
	multiline: boolean;
	remember: boolean;
	required: boolean;
	value: string;
	warning: string | null;
}
/** GAP: `SwitchCue` (an in-step cue such as "re-arms with Accept edits"). */
export interface SwitchCue {
	text: string;
}
/** GAP: `ChatChips`. */
export interface ChatChip {
	id: string;
	label: string;
	strong: boolean;
}
export interface ChatChips {
	chips: ChatChip[];
	link: string | null;
	linkLabel: string | null;
}
/** GAP: `OutView`. */
export interface OutView {
	path: RepoPath;
	exists: boolean;
}
export interface StepView {
	summary: StepSummary;
	prompt: PromptSegment[];
	inputs: RunInputView[];
	start: Explained<Start> | null;
	switches: SwitchCue[];
	chips: ChatChips | null;
	argvPreview: string[] | null;
	envNames: EnvName[];
	blockers: Blocker[];
	warnings: string[];
	remaps: string[];
	outs: OutView[];
	override: RunOverride | null;
	/** GAP: the step's optional parts, for the Include toggles (F-006). */
	parts: { partId: string; label: string; included: boolean; locked: boolean }[];
	/** GAP: the copied/paste-back state of a chat step. */
	copiedAt: Timestamp | null;
}
export type ActionKind = 'writeBlocks' | 'copyFile' | 'mkdir';
export interface ActionPreview {
	previewId: PreviewId;
	action: ActionKind;
	writes: Diff[];
}
export interface CopyResult {
	removedControls: number;
	focusRun: RunId | null;
}

// §4.5 chat
export interface ChatCopyResult {
	runId: RunId;
	opened: boolean;
	missingLink: 'project' | 'conversation' | null;
	removedControls: number;
}
export type LinkSource = { kind: 'clipboard' } | { kind: 'text'; url: string };
export type PasteSource = 'clipboard' | 'sensor';
/** GAP: `PacketFields`. */
export interface PacketFields {
	fromStage: Stage;
	toStage: Stage | null;
}
export interface PastePreview {
	previewId: PreviewId;
	kind: PasteKind;
	targets: Diff[];
	problems: Problem[];
	notes: string[];
	packetFields: PacketFields | null;
}
export interface PasteApplyResult {
	written: RepoPath[];
	stepDone: boolean;
}

// §4.6 launch
export type RearmNeed =
	| { kind: 'idleConfirmation'; chain: ChainId }
	| { kind: 'sensorBusy'; tasks: number }
	| { kind: 'exited'; chain: ChainId };
export interface LaunchPreview {
	planId: PlanId;
	expiresInMs: number;
	prompt: PromptSegment[];
	argv: string[];
	envNames: EnvName[];
	runSettingsSha256: Sha256;
	confirm: ConfirmReason[];
	rearm: RearmNeed | null;
	warnings: string[];
	remaps: string[];
}
export type PrepareOutcome =
	| ({ kind: 'ready' } & LaunchPreview)
	| { kind: 'blocked'; blockers: Blocker[]; warnings: string[] };
export type PrepareMode = 'normal' | 'freshWithContext';
export interface RunStarted {
	runId: RunId;
	chain: ChainId;
	tabName: string;
}
export type DoNextOutcome =
	| { kind: 'nothing'; why: string }
	| { kind: 'cli'; project: ProjectId; step: StepKey; prepare: PrepareOutcome }
	| { kind: 'chat'; project: ProjectId; step: StepKey; ready: boolean; blockers: Blocker[] };
export interface NextStepRow {
	project: ProjectSummary;
	next: StepSummary | null;
	whyNone: string | null;
	/** GAP: the miniature strip (P2) and the stage name shown with the step. */
	strip: SheetStatus[];
	stageTitle: string | null;
}
export interface TodayView {
	rows: NextStepRow[];
	inbox: InboxItem[];
}

// §4.2 doctor
export type DoctorScope = 'p1' | 'full';
/** GAP: `DoctorCheck`. */
export interface DoctorCheck {
	id: string;
	group: 'claude' | 'toolchain';
	name: string;
	level: DoctorLevel;
	summary: string | null;
	detail: TextSpan[];
	found: string | null;
	foundMono: boolean;
	action: 'captureLoginPath' | null;
}
/** GAP: `CapabilityRow`. */
export interface CapabilityRow {
	feature: string;
	code: boolean;
	minimum: string;
	found: boolean;
	blocks: string | null;
}
/** GAP: `ProbeOutcome`. */
export interface ProbeOutcome {
	probe: TextSpan[];
	result: 'pass' | 'fail' | 'inconclusive';
	text: string;
	detail: string | null;
}
export interface DoctorReport {
	level: DoctorLevel;
	checks: DoctorCheck[];
	capabilities: CapabilityRow[];
	/** GAP: the probe table and its footnote live in the report too. */
	probes: ProbeOutcome[];
	probeNote: string | null;
	ranAt: Timestamp;
}
/** GAP: `PathEntryCheck`. */
export interface PathEntryCheck {
	entry: string;
	ok: boolean;
	why: string | null;
}
export interface LoginPathPreview {
	previewId: PreviewId;
	shell: string;
	entries: PathEntryCheck[];
}
export interface ContractResult {
	version: string;
	verifiedAt: Timestamp;
	outcomes: ProbeOutcome[];
}
/** GAP: `Counts`. */
export interface Counts {
	stages: number;
	runnableSteps: number;
	items: number;
	errors: number;
	warnings: number;
}
/** GAP: `StageHealth`. */
export interface StageHealth {
	stages: string;
	title: string;
	status: 'runnable' | 'warning' | 'partial' | 'needsAnnotation';
	text: string;
	steps: string;
}
export interface Diagnostic {
	severity: 'error' | 'warning';
	rule: string;
	file: string;
	line: number;
	column: number;
	message: TextSpan[];
	/** GAP: the stage a diagnostic belongs to, for grouping. */
	stage: Stage;
}
export interface TemplateHealth {
	version: string;
	grammarVersion: number;
	counts: Counts;
	stages: StageHealth[];
	diagnostics: Diagnostic[];
	/** GAP: commit and file shown in the header. */
	commit: string;
	file: string;
}

// §4.7 terminal
export interface AttachInfo {
	status: RunStatus;
	replayedLines: number;
	tabName: string;
}
export interface PasteText {
	text: string;
	removedControls: number;
}
export type RearmOutcome =
	({ kind: 'relaunched' } & RunStarted) | { kind: 'endYourself'; chain: ChainId };
export type LinkOutcome = 'opened' | 'refused' | 'cancelled';

// §4.9 decisions
export type DecisionFileKind = 'project' | 'overlay' | 'lock';
export interface PendingChange {
	id: ChangeId;
	file: DecisionFileKind;
	detectedAt: Timestamp;
	seenSha256: Sha256;
	diff: RedactedDiff;
	/** GAP: the review's context and consequence lines (components.md "Outside-change review"). */
	effect: 'changesPath' | 'noEffect' | 'reloaded';
	consequences: { glyph: 'lock' | 'info' | 'check'; text: string }[];
	context: { changed: string; git: string; approved: string };
}
export interface RebuildPreview {
	previewId: PreviewId;
	stages: StageView[];
	lost: string[];
}

// §4.10 packets and run log
/** GAP: `PacketHeader`. */
export interface PacketHeader {
	line: string;
	fromStage: Stage;
	toStage: Stage | null;
}
/** GAP: `PacketFieldView`. */
export interface PacketFieldView {
	key: string;
	label: string;
	value: DocNode[];
	missing: boolean;
	problems: Problem[];
	note: TextSpan[] | null;
}
/** GAP: `NormalizationNote`. */
export interface NormalizationNote {
	field: string;
	message: string;
}
/** GAP: `Signature`. */
export interface Signature {
	name: string;
	date: IsoDate;
	byForeman: boolean;
	confirmed: boolean;
}
export interface PacketView {
	path: RepoPath;
	exists: boolean;
	sha256: Sha256 | null;
	header: PacketHeader | null;
	fields: PacketFieldView[];
	problems: Problem[];
	normalization: NormalizationNote[];
	signed: Signature | null;
	tamper: Diff | null;
	/** GAP: the raw file lines for the inspector (escaped spans per line). */
	raw: { line: number; spans: TextSpan[]; bad: boolean }[];
	/** GAP: when and by what the packet was written. */
	writtenBy: string | null;
	summary: string;
}
export type ExportResult = { kind: 'saved'; displayPath: string } | { kind: 'cancelled' };
export interface SurfaceMetrics {
	surface: Surface;
	medianTimeToStartMs: number;
	p95TimeToStartMs: number;
	medianActiveMsPerStep: number;
}
export interface StageEdits {
	stage: Stage;
	edits: number;
}
/** GAP: `BaselineComparison`. */
export interface BaselineComparison {
	baselineMedianMs: number;
	foremanMedianMs: number;
}
export interface MetricsView {
	perSurface: SurfaceMetrics[];
	manualEditsByStage: StageEdits[];
	stagesThroughForeman: number;
	baseline: BaselineComparison | null;
	runsCounted: number;
	/** GAP: p95 across all steps, the metric strip's third number (NFR-09 target 10 s). */
	p95AllMs: number;
}
export interface RunsPage {
	runs: RunSummary[];
	next: string | null;
	/** GAP: the total, for "5 of 212 runs". */
	total: number;
}

// §4.11 gates
/** GAP: `GateState`. */
export type GateState = 'notStarted' | 'inProgress' | 'failed' | 'passed' | 'signed';
/** GAP: `GateItemView`. */
export interface GateArtifact {
	kind: 'path' | 'command';
	text: string;
}
export interface GateItemView {
	id: ItemId;
	text: string;
	kind: 'attest' | 'time';
	how: string | null;
	status: 'pass' | 'fail' | 'waiting' | 'notApplicable';
	evidence: TextSpan[] | null;
	proposed: 'pass' | 'fail' | null;
	note: string | null;
	waitsFor: string | null;
	artifacts: GateArtifact[];
	failures: number;
	needsReconfirm: boolean;
	session: string | null;
}
/** GAP: `CloseoutStep`. */
export interface CloseoutStep {
	key: 'gateA' | 'results' | 'gateB' | 'gateC' | 'sign';
	label: string;
	state: 'done' | 'now' | 'failed' | 'pending' | 'locked';
	note: string | null;
}
export interface GateView {
	stage: Stage;
	state: GateState;
	items: GateItemView[];
	closeout: CloseoutStep[];
	signable: boolean;
	whyNot: string[];
	/** GAP: "Imported from … at 14:12" under the gate actions. */
	importedFrom: string | null;
}
export type AttestOutcome =
	| { kind: 'done'; note: string; link: string | null }
	| { kind: 'notApplicable'; reason: string }
	| { kind: 'clear' };
export interface ImportProposal {
	item: ItemId;
	proposed: 'pass' | 'fail' | 'unmatched';
	evidence: TextSpan[];
}

// §4.12 signing
export type SignBranch = { kind: 'current' } | { kind: 'other'; branch: string };
export interface SignPreview {
	previewId: PreviewId;
	packetSha256: Sha256;
	short: string;
	addedFields: Diff;
	paths: RepoPath[];
	confirmConditions: ConditionId[] | null;
	chainBusy: boolean;
	blockers: string[];
	/** GAP: the current branch, the commit message and pending notes for the sign sheet. */
	branch: string;
	commitMessage: string;
	pathNotes: Record<string, string>;
}
export type SignResult =
	| { kind: 'signed'; commit: string; stage: Stage }
	| { kind: 'commitPending'; stderrTail: string }
	| { kind: 'readBackMismatch'; diffs: Diff[] };

// §4.13 lessons, manifest, assembler
/** GAP: `LessonRef`. */
export interface LessonRef {
	id: string;
	title: string;
}
export interface LessonsStatus {
	met: boolean;
	entries: LessonRef[];
	nextId: string;
	filePresent: boolean;
}
/** GAP: `ManifestRow`. */
export interface ManifestRow {
	path: RepoPath;
	exists: boolean;
	inProject: boolean;
	producer: StepKey | null;
}
export interface Manifest {
	files: ManifestRow[];
	ready: number;
	total: number;
	firstProducer: StepKey | null;
}
export interface AssemblerRow {
	path: string;
	state: 'exists' | 'missing' | 'empty' | 'refused';
	rule: string | null;
	/** GAP: the row's note (root CLAUDE.md, empty file). */
	note: string | null;
}
/** GAP: `LateConditionView`. */
export interface LateConditionView {
	condition: ConditionId;
	value: TriState;
	proposed: TriState | null;
	evidence: string | null;
	decided: string | null;
}
export interface Assembler {
	rows: AssemblerRow[];
	complete: boolean;
	lateConditions: LateConditionView[];
}

// §4.14 desktop, overlay, run sheet
export interface DesktopOpened {
	chips: ChatChips;
}
export interface AddedStepConfig {
	surface: Surface;
	cwd: CwdKind | null;
	start: Start;
	out: string[];
}

// §5 channels
export type LaunchProgress =
	| { phase: 'version'; found: string }
	| { phase: 'contract'; outcome: string }
	| { phase: 'fingerprint'; firstScan: boolean }
	| { phase: 'auth'; cached: boolean }
	| { phase: 'transcript' }
	| { phase: 'inputs' }
	| { phase: 'ending'; step: 'ctrlC' | 'exit' | 'waiting' }
	| { phase: 'exited'; code: number }
	| { phase: 'spawning' }
	| { phase: 'done' };
export type SignProgress =
	| { step: 'reRead' }
	| { step: 'write'; path: RepoPath }
	| { step: 'hashObject'; path: RepoPath }
	| { step: 'writeTree' }
	| { step: 'commitTree'; signed: boolean }
	| { step: 'updateRef' }
	| { step: 'refreshIndex' }
	| { step: 'readBack'; path: RepoPath; ok: boolean }
	| { step: 'done'; commit: string };
/** Raw PTY bytes (`Channel<tauri::ipc::Response>` → ArrayBuffer). */
export type PtyOutput = (chunk: ArrayBuffer) => void;
export type OnProgress = (p: LaunchProgress) => void;
export type OnSignProgress = (p: SignProgress) => void;

// §6 events: identifiers and revisions only.
export interface EventPayloads {
	'project:changed': { projectId: ProjectId; rev: Rev };
	'projects:changed': { rev: Rev };
	'packet:changed': { projectId: ProjectId; stage: Stage; rev: Rev };
	'decision:pending': { projectId: ProjectId; count: number };
	'tamper:detected': { projectId: ProjectId; stage: Stage; rev: Rev };
	'run:status': { runId: RunId; status: RunStatus; rev: Rev };
	'config:changed': { projectId: ProjectId; cwd: CwdKind };
	'doctor:status': { level: DoctorLevel };
	'settings:changed': { rev: Rev };
}
export type EventName = keyof EventPayloads;
export const EVENT_NAMES = [
	'project:changed',
	'projects:changed',
	'packet:changed',
	'decision:pending',
	'tamper:detected',
	'run:status',
	'config:changed',
	'doctor:status',
	'settings:changed'
] as const satisfies readonly EventName[];
export type Unlisten = () => void;
