// The blueprint as the mock knows it: 19 stages, the annotated steps of Stages 0–5 (blueprint 2.2,
// PROPOSAL.md B-1) and the prompt titles of 6–18, which need annotation. Mock-only data.
import type { ConditionId, Stage, StepKind, Surface, Tag } from '../types.ts';

export const STAGE_TITLES = [
	'Land Survey',
	'Meet the Architect',
	"Architect's Design Studio",
	'Permits & Zoning',
	'Construction Drawings',
	'Site Prep & Excavation',
	'Foundation',
	'Framing',
	'Roofing',
	'Plumbing',
	'Electrical',
	'HVAC',
	'Insulation & Drywall',
	'Siding & Exterior',
	'Inspections',
	'Paint & Trim',
	'Final Walkthrough',
	'Certificate of Occupancy',
	'Homeowner Maintenance'
] as const;

export const STAGE_SUBTITLES: readonly string[] = [
	'Find where the money is.',
	'Interview, brainstorm, and write the App Brief in one claude.ai chat.',
	'Design the entire product. Council in Claude Code, then freeze the Master Spec.',
	'Business, security, privacy, legal.',
	'The complete build package.',
	'Repo, CI, environments.',
	'Database, auth, domain core.',
	'The walking skeleton.',
	'Security controls from the threat model.',
	'Data flows and integrations.',
	'Payments and entitlements.',
	'Observability, SLOs, and error budgets.',
	'Complete every feature.',
	'Design system, polish, accessibility.',
	'Independent audits.',
	'Copy, onboarding, growth surfaces.',
	'Readiness review, drills, dogfood, beta.',
	'Ring-based launch and revenue.',
	'Operate and grow.'
];

export interface TemplateStep {
	key: string;
	number: string;
	title: string;
	kind: StepKind;
	surface: Surface | 'action';
	tag: Tag;
	when: ConditionId[] | null;
	/** Spec-line pieces (display form). */
	where: string;
	model: string | null;
	effort: string | null;
	mode: string | null;
	agents: string[];
	note: string | null;
}

function s(
	key: string,
	title: string,
	kind: StepKind,
	surface: Surface | 'action',
	tag: Tag,
	where: string,
	model: string | null,
	effort: string | null,
	mode: string | null,
	extra: Partial<TemplateStep> = {}
): TemplateStep {
	const number = key === 'council-setup' ? '2.0' : key.includes(':F.2') ? 'F.2' : key;
	return {
		key,
		number,
		title,
		kind,
		surface,
		tag,
		when: null,
		where,
		model,
		effort,
		mode,
		agents: [],
		note: null,
		...extra
	};
}

const CHAT1 = 'Chat, same chat as 1.1';
const CLI2 = 'CLI, continues 2.2';

export const TEMPLATE: Record<number, TemplateStep[]> = {
	0: [
		s(
			'0.1',
			'Opportunity scan',
			'prompt',
			'chat',
			'optional',
			'Chat, new conversation',
			'Opus 5.5',
			'adaptive',
			null,
			{
				when: ['idea_known']
			}
		),
		s(
			'0.2',
			'Red-team and pick',
			'followup',
			'chat',
			'optional',
			'Chat, same chat as 0.1',
			'Opus 5.5',
			'adaptive',
			null,
			{
				when: ['idea_known']
			}
		)
	],
	1: [
		s(
			'1.1',
			'The architect interview',
			'prompt',
			'chat',
			'core',
			'Chat, new conversation',
			'Opus 5.5',
			'adaptive',
			null
		),
		s(
			'1.2',
			'Brainstorm and converge',
			'followup',
			'chat',
			'core',
			CHAT1,
			'Opus 5.5',
			'adaptive',
			null
		),
		s(
			'1.3',
			'Demand validation kit',
			'followup',
			'chat',
			'conditional',
			CHAT1,
			'Opus 5.5',
			'adaptive',
			null,
			{
				when: ['commercial']
			}
		),
		s('1.4', 'The App Brief', 'followup', 'chat', 'core', CHAT1, 'Opus 5.5', 'adaptive', null, {
			note: 'writes the Stage 1 packet'
		}),
		s('1:F.2', 'Lessons capture', 'lessons', 'chat', 'core', CHAT1, 'Opus 5.5', 'adaptive', null, {
			note: 'appends docs/lessons.md'
		})
	],
	2: [
		s('council-setup', 'Council setup', 'action', 'action', 'core', 'Action', null, null, null, {
			note: 'Foreman writes spec/.claude/agents/'
		}),
		s(
			'2.1',
			'Competitor teardown',
			'prompt',
			'chat',
			'conditional',
			'Chat, Research on',
			'Opus 5.5',
			'adaptive',
			null,
			{
				when: ['commercial']
			}
		),
		s(
			'2.2',
			'Convene the council',
			'prompt',
			'cli',
			'core',
			'CLI in spec/',
			'Fable 5.1',
			'high',
			'Plan, then Accept edits',
			{
				agents: ['principal-engineer', 'security-architect', 'user-advocate', 'product-strategist']
			}
		),
		s(
			'2.3',
			'Feature specifications',
			'followup',
			'cli',
			'core',
			CLI2,
			'Fable 5.1',
			'high',
			'Accept edits'
		),
		s(
			'2.4',
			'Stack decision, evidence first',
			'followup',
			'cli',
			'core',
			CLI2,
			'Fable 5.1',
			'high',
			'Accept edits',
			{
				agents: ['principal-engineer'],
				note: null
			}
		),
		s(
			'x-2-1',
			'Re-rank pain with interview notes',
			'followup',
			'cli',
			'added',
			CLI2,
			'Fable 5.1',
			'high',
			'Accept edits',
			{
				number: 'x‑2‑1',
				agents: ['user-advocate']
			}
		),
		s(
			'2.5',
			'Non-functional requirements',
			'followup',
			'cli',
			'core',
			CLI2,
			'Fable 5.1',
			'high',
			'Accept edits'
		),
		s(
			'2.6',
			'Freeze the Master Spec',
			'followup',
			'cli',
			'core',
			CLI2,
			'Fable 5.1',
			'high',
			'Manual'
		),
		s(
			'2:F.2',
			'Lessons capture',
			'lessons',
			'cli',
			'core',
			CLI2,
			'Fable 5.1',
			'high',
			'Accept edits',
			{
				note: 'writes docs/lessons.md'
			}
		)
	],
	3: [
		s(
			'3.1',
			'Business model and pricing',
			'prompt',
			'chat',
			'conditional',
			'Chat, new conversation',
			'Opus 5.5',
			'adaptive',
			null,
			{
				when: ['commercial', 'payments']
			}
		),
		s(
			'3.2',
			'Security, privacy, and compliance requirements',
			'prompt',
			'chat',
			'core',
			'Chat, new conversation',
			'Opus 5.5',
			'adaptive',
			null
		),
		s(
			'3:F.2',
			'Lessons capture',
			'lessons',
			'chat',
			'core',
			'Chat, same chat as 3.2',
			'Opus 5.5',
			'adaptive',
			null
		)
	],
	4: [
		s(
			'4.1',
			'Scope, milestones, and acceptance criteria',
			'prompt',
			'chat',
			'core',
			'Chat, new conversation in Project',
			'Opus 5.5',
			'adaptive',
			null
		),
		s(
			'4.2',
			'Design doc and ADRs',
			'prompt',
			'chat',
			'core',
			'Chat, new conversation in Project',
			'Opus 5.5',
			'adaptive',
			null
		),
		s(
			'4.3',
			'Threat model',
			'prompt',
			'chat',
			'core',
			'Chat, new conversation in Project',
			'Opus 5.5',
			'adaptive',
			null
		),
		s(
			'4.4',
			'Data model, API contract, domain rules',
			'prompt',
			'chat',
			'core',
			'Chat, new conversation in Project',
			'Opus 5.5',
			'adaptive',
			null
		),
		s(
			'4.5',
			'Design system and key screens',
			'prompt',
			'chat',
			'conditional',
			'Chat, new conversation in Project',
			'Opus 5.5',
			'adaptive',
			null,
			{
				when: ['has_ui'],
				note: 'Research off'
			}
		),
		s(
			'4.6',
			'Tickets',
			'prompt',
			'chat',
			'core',
			'Chat, new conversation in Project',
			'Opus 5.5',
			'adaptive',
			null
		),
		s(
			'4.7',
			'Independent design review',
			'prompt',
			'chat',
			'core',
			'Chat, new conversation in Project',
			'Opus 5.5',
			'adaptive',
			null
		),
		s(
			'4.8',
			'CLAUDE.md and the Build Package',
			'prompt',
			'chat',
			'core',
			'Chat, new conversation in Project',
			'Opus 5.5',
			'adaptive',
			null,
			{
				note: 'writes the Stage 4 packet'
			}
		),
		s(
			'4:F.2',
			'Lessons capture',
			'lessons',
			'chat',
			'core',
			'Chat, same chat as 4.8',
			'Opus 5.5',
			'adaptive',
			null
		)
	],
	5: [
		s('5.1', 'Scaffold', 'prompt', 'cli', 'core', 'CLI in project', 'Opus 5.5', 'high', 'Plan'),
		s(
			'5.2',
			'CI with security gates',
			'followup',
			'cli',
			'core',
			'CLI, continues 5.1',
			'Opus 5.5',
			'high',
			'Accept edits'
		),
		s(
			'5.3',
			'ADR reality check',
			'prompt',
			'cli',
			'core',
			'CLI in project',
			'Opus 5.5',
			'high',
			'Plan'
		),
		s(
			'5.4',
			'Automation and preview config',
			'prompt',
			'cli',
			'core',
			'CLI in project',
			'Sonnet 5.5',
			'medium',
			'Accept edits'
		),
		s(
			'5.5',
			'Staging pipeline',
			'prompt',
			'cli',
			'conditional',
			'CLI in project',
			'Opus 5.5',
			'high',
			'Accept edits',
			{
				when: ['server']
			}
		),
		s(
			'5:gate.A',
			'Gate A: inspection',
			'gate',
			'cli',
			'core',
			'CLI in project',
			'Opus 5.5',
			'high',
			'Plan',
			{
				number: 'A',
				agents: ['test-runner', 'security-reviewer']
			}
		)
	]
};

/** Prompt titles of the stages blueprint 2.2 doesn't annotate yet (shown, never runnable). */
export const UNANNOTATED: Record<number, string[]> = {
	6: ['Migrations', 'Authentication', 'Domain core'],
	7: ['Skeleton plan'],
	8: ['Security envelope'],
	9: ['Data flows'],
	10: ['Billing'],
	11: ['Observability'],
	12: ['Milestone sweep'],
	13: ['Apply the design system', 'Accessibility'],
	14: [
		'Final security review and penetration test',
		'Load, capacity, and failure modes',
		'Architecture audit'
	],
	15: ['Copy and onboarding', 'Implementation'],
	16: ['Production Readiness Review and launch review', 'Drills'],
	17: ['Production rollout plan', 'Ring health review'],
	18: ['Production incident', 'Blameless postmortem', 'Change Request', 'Monthly ADR hygiene']
};

export const PHASES: { key: string; label: string; from: Stage; to: Stage }[] = [
	{ key: 'pre', label: 'Pre-construction', from: 0, to: 4 },
	{ key: 'structure', label: 'Structure', from: 5, to: 8 },
	{ key: 'systems', label: 'Systems', from: 9, to: 11 },
	{ key: 'finish', label: 'Finishes', from: 12, to: 15 },
	{ key: 'co', label: 'Occupancy', from: 16, to: 17 },
	{ key: 'maint', label: 'Ownership', from: 18, to: 18 }
];

export const CONDITIONS: {
	id: ConditionId;
	label: string;
	effect: string;
	late: boolean;
}[] = [
	{
		id: 'idea_known',
		label: 'You already know the idea',
		effect: 'Yes skips Stage 0, Land Survey.',
		late: false
	},
	{
		id: 'commercial',
		label: "You'll sell it",
		effect: 'Adds the demand test, competitor teardown, pricing and marketing pages.',
		late: false
	},
	{
		id: 'payments',
		label: 'It takes payments',
		effect: 'Adds Stage 10, Electrical, and live payments at launch.',
		late: false
	},
	{
		id: 'multi_user',
		label: 'More than one person signs in',
		effect: 'Adds authentication, onboarding, the beta ring and analytics.',
		late: false
	},
	{
		id: 'public',
		label: 'Anyone on the internet can reach it',
		effect: 'Adds authentication hardening and public-facing reviews.',
		late: false
	},
	{
		id: 'has_ui',
		label: 'It has a user interface',
		effect: 'Adds 4.5 design system and Stage 13 accessibility.',
		late: false
	},
	{
		id: 'others_data',
		label: "It stores other people's data",
		effect: 'Adds the legal part of 3.2 and legal pages.',
		late: false
	},
	{
		id: 'server',
		label: 'It runs on a server',
		effect: 'Adds the staging pipeline, SLOs and load tests.',
		late: false
	},
	{ id: 'has_database', label: 'It has a database', effect: 'Adds 6.1 migrations.', late: true },
	{
		id: 'has_uploads',
		label: 'It takes uploads',
		effect: 'Adds the uploads part of Stage 9.',
		late: true
	},
	{
		id: 'has_email',
		label: 'It sends email',
		effect: 'Adds the email part of Stage 9.',
		late: true
	},
	{
		id: 'has_integrations',
		label: 'It integrates other services',
		effect: 'Adds integration tests to Stage 9.',
		late: true
	}
];

export interface VariableSpec {
	name: string;
	label: string;
	placeholder: string;
	multiline: boolean;
	fromSettings: boolean;
}
export const VARIABLES: VariableSpec[] = [
	{
		name: 'ROUGH_IDEA',
		label: 'Rough idea',
		placeholder: '<one or two sentences>',
		multiline: true,
		fromSettings: false
	},
	{
		name: 'TARGET_USERS',
		label: "Who it's for",
		placeholder: "<who it's for>",
		multiline: false,
		fromSettings: false
	},
	{
		name: 'PREFERRED_STACK',
		label: 'Preferred stack',
		placeholder: '<your stack>',
		multiline: false,
		fromSettings: true
	},
	{
		name: 'QUALITY_BAR',
		label: 'Quality bar',
		placeholder: '<quality bar>',
		multiline: false,
		fromSettings: true
	}
];
