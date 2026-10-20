// Prompt text for the mock's steps, in a tiny markup the mock resolves the way Rust's resolver
// would (DR-080 ff.):  {{TOKEN|value|note}} a resolved project variable, [[input:name]] a run input,
// <<part:id|text>> an optional part. Mock-only data.
export const PROMPTS: Record<string, string> = {
	'1.1':
		'You are a principal software architect and product strategist. I want to build {{APP_NAME|TradeLog|APP_NAME from project.json}}.\nRough idea: {{ROUGH_IDEA|A trading journal that imports fills from my broker and shows which setups actually make money.|ROUGH_IDEA from project.json}}\nInterview me one question at a time until you could write the App Brief yourself.',
	'1.2':
		'Brainstorm three directions for {{APP_NAME|TradeLog|APP_NAME from project.json}}, then converge on one with me.\nFor each, give the user, the job it does, and what would make it fail.',
	'1.3':
		'Design a demand validation kit for {{APP_NAME|TradeLog|APP_NAME from project.json}}: a landing page, a paid test and a threshold.',
	'1.4':
		'Here are the validation results: [[input:numbers]]\nIf they missed the threshold, tell me plainly and stop. Otherwise,\n<<part:press-release|write a press release for launch day as if the product already exists\n(Amazon "working backwards" style): headline, the problem, the solution,\na customer quote, and how to get started. Add an FAQ of the 8 hardest\nquestions a customer and an investor would ask, with answers.>>\nThen write the App Brief and the Stage 1 handoff packet.',
	'2.1':
		'Tear down every serious competitor of {{APP_NAME|TradeLog|APP_NAME from project.json}}: features, pricing, reviews.',
	'2.2':
		'You are the chair of the product design council for {{APP_NAME|TradeLog|APP_NAME from project.json}}.\nRead docs/handoff/stage-01.md and spec/01-app-brief.md. Run five rounds: positions, critique,\nred team, resolution, feature specs. Write each round to council/.',
	'2.3':
		'ultrathink. For each v1 feature, write a specification a senior engineer could build from:\nstarting situation, behavior, inputs and outputs, edge cases, done criteria.\n-> council/r6-feature-specs.md',
	'2.4':
		'Recommend the best stack for {{APP_NAME|TradeLog|APP_NAME from project.json}}, layer by layer: frontend,\nbackend, data, cache/queue, search, file storage, auth, payments,\nemail, hosting, observability, CI/CD.\n\nPART 1, evidence-first (ignore my preferences): for each layer, list the\nrequirements it must meet (from the feature specs and non-functional\nneeds), the 2-3 strongest candidates, and the winner. …\n\nPART 2, compare with my preference: {{PREFERRED_STACK|SvelteKit + Rust/Axum + Postgres|PREFERRED_STACK from project.json}}.\nFor each layer where you differ, say what I\'d gain, what I\'d lose, and\nyour confidence (high / medium / low). …\n\nPART 3, cite the evidence: official docs, release notes and what top\nengineering teams ship in production. Mark anything you couldn\'t verify.\n\nPART 4, the final table: layer, choice, runner-up, why, risk.\nAsk me to confirm each layer, then record my final choice per layer.\nDon\'t change any other council file.\n-> council/r7-stack.md',
	'x-2-1':
		'Here are my interview notes: [[input:notes]]\nCompare them with the pain map in council/r1-positions.md.\n<<part:rerank|Ask user-advocate to re-rank the pain map from these notes and say what moved and why.>>\nDon\'t change the feature specs.',
	'2.5':
		'Write the non-functional requirements for {{APP_NAME|TradeLog|APP_NAME from project.json}}: performance, reliability,\nsecurity, privacy, accessibility, each with a measurable target.\n-> council/r8-nfrs.md',
	'2.6':
		'Freeze the Master Spec: merge the council rounds into spec/MASTER-SPEC.md, mark it FROZEN v1.0,\nand write the Stage 2 handoff packet to docs/handoff/stage-02.md.',
	'2:F.2':
		'Capture the lessons from Stage 2 in docs/lessons.md: what went wrong, the root cause and the fix.',
	'4.5':
		'Using the Master Spec and the design direction, create the design system for {{APP_NAME|Foreman|APP_NAME from project.json}}:\ntokens, components with states, and mockups of the key screens in both themes.',
	'5.1':
		'Scaffold the repository exactly as the design doc says. Tests first; nothing beyond the scaffold.'
};

export const DEFAULT_PROMPT = (title: string): string =>
	`${title}.\nFollow the blueprint for this step and write the output it names.`;
