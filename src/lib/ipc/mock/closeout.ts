// The P1 close-out checklist (AC-F015-01), mirroring `foreman_core::steps::closeout::sequence`.

/** The close-out sequence for a stage's type (MASTER-SPEC §8), worded for doing it by hand. */
export function closeoutSequence(stage: number): string[] {
	const nn = String(stage).padStart(2, '0');
	const sign = `Sign: write APPROVED BY: <your name> on <date> in docs/handoff/stage-${nn}.md, confirm the signature in Foreman, then commit the stage`;
	const attest = "Attest the inspection items: a note for each, or why it doesn't apply";
	if ([0, 1, 3, 4].includes(stage))
		return [
			`Packet pasted back and valid (docs/handoff/stage-${nn}.md)`,
			attest,
			`F.2 lessons in the same chat; add the entries to docs/lessons.md yourself (Stage ${stage})`,
			sign
		];
	if (stage === 2)
		return [
			"2.6 writes the packet (docs/handoff/stage-02.md) and it's valid",
			attest,
			'F.2 as a followup in the 2.2 chain, writing to docs/lessons.md',
			sign
		];
	if (stage >= 5 && stage <= 17)
		return [
			`Gate A (interactive): run ${stage}:gate.A and confirm the results`,
			`Gate B writes the packet (docs/handoff/stage-${nn}.md) and it's valid`,
			`Gate C lessons: run ${stage}:gate.C, writing to docs/lessons.md`,
			sign
		];
	return [];
}
