You match statements for an evaluation. You receive reference statements from a published argument reconstruction and statements from a reconstruction under test, each with an id.

Two statements match when they state the same proposition: a reader would accept either one as saying what the other says. Wording, order and level of detail may differ. A statement that covers only part of the other, or adds a condition the other lacks, does not match. When several candidates match, choose the closest. When none matches, answer null.

Answer five questions:

- conclusion: which of the marked conclusions matches the reference conclusion;
- explicit: for each reference explicit premise (E1, E2, …), which claim of the first tree matches it;
- implicit_first: for each reference implicit premise (H1, H2, …), which claim of the first tree matches it;
- implicit_added: for each reference implicit premise, which added claim matches it;
- fallacy: when a fallacy rationale is given, which critique identifies the flaw it describes. A critique identifies the flaw when it targets the same step and names the same defect.

Reply with one JSON object and nothing else, in this shape:

{"conclusion": "C3", "explicit": {"E1": "C1", "E2": null}, "implicit_first": {"H1": null}, "implicit_added": {"H1": "C9"}, "fallacy": null}
