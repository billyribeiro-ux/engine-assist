// The new-project form's state (F-018), shared by the P1 single form and the P2 wizard: the same
// fields, the same checks, the same preview and create (AC-F018-01). Rust re-validates everything;
// the folder comes only from Rust's dialog (setup_pick_folder), never from typed text (SR-24).
import { slugOf } from '#lib/actions/slug.ts';
import { failureText } from '#lib/actions/words.ts';
import { toIpcFailure } from '#lib/ipc/errors.ts';
import { ipc } from '#lib/ipc/index.ts';
import type {
	ConditionId,
	ConditionSpec,
	FolderPick,
	ProjectSummary,
	SetupPreview,
	TriState,
	VariableSpec
} from '#lib/ipc/types.ts';
import { refresh } from '#lib/stores/bus.ts';

export class SetupForm {
	name = $state('');
	url = $state('');
	pick = $state<FolderPick | null>(null);
	pickError = $state<string | null>(null);
	variables = $state<Record<string, string>>({});
	conditions = $state<Partial<Record<ConditionId, TriState>>>({});
	specs = $state<VariableSpec[]>([]);
	conditionSpecs = $state<ConditionSpec[]>([]);
	defaults = $state<Record<string, string>>({});
	preview = $state<SetupPreview | null>(null);
	previewError = $state<string | null>(null);
	createError = $state<string | null>(null);
	touched = $state<Record<string, boolean>>({});
	busy = $state(false);
	private timer: ReturnType<typeof setTimeout> | undefined;

	async load(): Promise<void> {
		const c = await ipc();
		const s = await c.settingsGet({});
		const bp = s.blueprintRepo;
		this.specs = bp?.variables ?? [];
		this.conditionSpecs = (bp?.conditions ?? []).filter((x) => !x.late);
		this.defaults = s.globalVariables;
		for (const v of this.specs)
			this.variables[v.name] ??= v.fromSettings ? (s.globalVariables[v.name] ?? '') : '';
		for (const x of this.conditionSpecs) this.conditions[x.id] ??= 'undecided';
	}

	get slug(): string {
		return slugOf(this.name);
	}

	/** Field errors, by field id; placeholders count as empty (AC-F018-01). */
	get errors(): Record<string, string> {
		const e: Record<string, string> = {};
		if (!this.name.trim()) e.name = 'Name the project.';
		else if (!this.slug) e.name = 'Use at least one letter or digit in the name.';
		if (!this.pick) e.folder = 'Choose the project folder.';
		else if (this.pick.refusal) e.folder = this.pick.refusal;
		if (this.url && !/^https:\/\/claude\.ai\/[^\s@]*$/.test(this.url))
			e.url = 'Use a https://claude.ai/ link to the Project.';
		for (const v of this.specs) {
			const val = (this.variables[v.name] ?? '').trim();
			if (!val) e[`var-${v.name}`] = `Describe ${v.label.toLowerCase()} in your words.`;
			else if (val === v.placeholder)
				e[`var-${v.name}`] =
					`This is still the blueprint's placeholder. Describe ${v.label.toLowerCase()} in your words.`;
		}
		return e;
	}
	get fieldsToFix(): string[] {
		const names: Record<string, string> = {
			name: 'Name',
			folder: 'Folder',
			url: 'claude.ai Project link'
		};
		return Object.keys(this.errors).map((k) =>
			k.startsWith('var-')
				? (this.specs.find((v) => `var-${v.name}` === k)?.label ?? k)
				: (names[k] ?? k)
		);
	}
	get valid(): boolean {
		return Object.keys(this.errors).length === 0 && !!this.preview;
	}

	async choose(): Promise<void> {
		const c = await ipc();
		this.pickError = null;
		try {
			this.pick = await c.setupPickFolder({});
			this.touched.folder = true;
			this.schedule(0);
		} catch (e: unknown) {
			const f = toIpcFailure(e);
			this.pickError = f.code === 'cancelled' ? null : failureText(f);
		}
	}

	/** Re-previews shortly after a change; the preview is what Create writes. */
	schedule(ms = 250): void {
		clearTimeout(this.timer);
		this.timer = setTimeout(() => void this.refreshPreview(), ms);
	}

	async refreshPreview(): Promise<void> {
		if (!this.pick) return;
		const c = await ipc();
		const variables: Record<string, string> = {};
		for (const [k, v] of Object.entries(this.variables)) if (v.trim()) variables[k] = v;
		try {
			this.preview = await c.setupPreview({
				pickId: this.pick.pickId,
				name: this.name.trim() || 'project',
				slug: this.slug || 'project',
				variables,
				conditions: this.conditions,
				chatProjectUrl: this.url && !this.errors.url ? this.url : null
			});
			this.previewError = null;
		} catch (e: unknown) {
			this.previewError = failureText(toIpcFailure(e));
		}
	}

	async create(): Promise<ProjectSummary | null> {
		if (!this.valid || !this.preview) return null;
		await this.refreshPreview();
		const c = await ipc();
		this.busy = true;
		try {
			const p = await c.setupApply({ previewId: this.preview.previewId });
			refresh('projects', 'today');
			return p;
		} catch (e: unknown) {
			this.createError = failureText(toIpcFailure(e));
			return null;
		} finally {
			this.busy = false;
		}
	}
}
