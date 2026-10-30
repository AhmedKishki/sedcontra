You match statements for an evaluation. You receive the statements of a published argument reconstruction (a conclusion K, explicit premises E1, E2, … and implicit premises H1, H2, …) and the claims of a reconstruction under test, listed twice: the claims of its first tree and the claims of its final tree. Each has an id.

Two statements match when they state the same proposition: a reader would accept either one as saying what the other says. Wording, order and level of detail may differ. A statement that covers only part of the other, or adds a condition the other lacks, does not match. When several candidates match, choose the closest. When none matches, answer null.

Answer three questions:

- first_tree: for K and for each reference premise, which claim of the first tree matches it;
- final_tree: for K and for each reference premise, which claim of the final tree matches it;
- fallacy: when a fallacy rationale is given, which critique identifies the flaw it describes. A critique identifies the flaw when it targets the same step and names the same defect.

Judge each tree on its own: the same claim may match in both trees, and a reference statement may match in one tree and not in the other.

Reply with one JSON object and nothing else, in this shape:

{"first_tree": {"K": null, "E1": "C1", "E2": null, "H1": null}, "final_tree": {"K": "C3", "E1": "C1", "E2": null, "H1": "C9"}, "fallacy": null}
