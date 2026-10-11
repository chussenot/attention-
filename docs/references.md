# References

Each entry says why it matters to this project. Identifiers were checked on 2026-10-11:
arXiv IDs against the arXiv export API, DOIs against Crossref. Entries without an identifier
are books or broad programmes cited from memory and are marked as such.

Verification legend: ✅ identifier resolved to the cited work · 📚 book or programme, no
identifier checked.

## Core: the mechanism to implement

- ✅ L. Blum & M. Blum, *A theory of consciousness from a theoretical computer science
  perspective: Insights from the Conscious Turing Machine*. arXiv:2107.13704; PNAS 119(21),
  2022, doi:10.1073/pnas.2115934119. — **Primary spec**: processors, the competition tree,
  global broadcast, links. Our `Workspace` is a deterministic, single-chunk reading of it.
- 📚 B. J. Baars, *A Cognitive Theory of Consciousness*, Cambridge University Press, 1988. —
  Origin of Global Workspace Theory: specialists compete for a limited-capacity stage whose
  content is broadcast to all.
- ✅ S. Dehaene, M. Kerszberg & J.-P. Changeux, *A neuronal model of a global workspace in
  effortful cognitive tasks*, PNAS 95(24), 1998, doi:10.1073/pnas.95.24.14529; and S. Dehaene
  & J.-P. Changeux, *Experimental and theoretical approaches to conscious processing*, Neuron
  70(2), 2011, doi:10.1016/j.neuron.2011.03.018. — Global Neuronal Workspace: ignition and
  limited capacity, i.e. why selection is winner-take-all rather than a soft blend.
- ✅ A. Goyal et al., *Coordination Among Neural Modules Through a Shared Global Workspace*,
  arXiv:2103.01197. — A learned, limited-capacity shared workspace that specialist modules
  compete to write to; evidence that the bottleneck itself is useful.
- ✅ R. VanRullen & R. Kanai, *Deep Learning and the Global Workspace Theory*,
  arXiv:2012.10390. — Architectural roadmap for building a GWT-style system from modules.

## Attention as selection, and attention about attention

- ✅ Y. Bengio, *The Consciousness Prior*, arXiv:1709.08568. — A sparse bottleneck selecting a
  few elements from a large latent state; frames selection as an inductive bias.
- ✅ A. Goyal et al., *Recurrent Independent Mechanisms*, arXiv:1909.10893. — Modules compete
  for input via attention; a precedent for competition-based routing.
- ✅ A. Goyal & Y. Bengio, *Inductive Biases for Deep Learning of Higher-Level Cognition*,
  arXiv:2011.15091. — Survey motivating modularity, sparse selection and broadcast.
  *Corrected:* the brief's title said "…higher-level human cognition"; the arXiv title is
  "…Higher-Level Cognition".
- 📚 M. S. A. Graziano, Attention Schema Theory (research programme). — An agent maintains a
  simplified model of its own attention in order to control it. Basis for P3.
- ✅ D. Liu, S. Bolotta, H. Zhu, Y. Bengio & G. Dumas, *Attention Schema in Neural Agents*,
  arXiv:2305.17375. — *Previously ⚠, now verified.* Agents with an attention schema used as
  recurrent internal control performed best in multi-agent RL; a design pointer for P3.
- ✅ A. I. Wilterson & M. S. A. Graziano, *The attention schema theory in a neural network
  agent: Controlling visuospatial attention using a descriptive model of attention*, PNAS
  118(33), 2021, doi:10.1073/pnas.2102421118. — Shows that a model of attention improves
  control of attention; the P3 hypothesis in its simplest form.
- ✅ A. Vaswani et al., *Attention Is All You Need*, arXiv:1706.03762. — Context only:
  transformer attention is contextual weighting, not the competitive, capacity-limited
  selection this crate studies.

## Evaluation and framing

- ✅ P. Butlin, R. Long et al., *Consciousness in Artificial Intelligence: Insights from the
  Science of Consciousness*, arXiv:2308.08708. — Map of the field: indicator properties derived
  per theory (e.g. GWT's limited-capacity workspace with global broadcast). We use it to locate
  mechanisms, not to score systems.
- ✅ D. J. Chalmers, *Could a Large Language Model be Conscious?*, arXiv:2303.07103 (Boston
  Review, 2023). — Framing; explains why the "is it conscious?" question is out of scope here.
- ✅ A. Juliani, K. Arulkumaran, S. Sasai & R. Kanai, *On the link between conscious function
  and general intelligence in humans and machines*, arXiv:2204.05133. — *Previously ⚠, now
  verified.* Links GWT-like functions to general intelligence; supports evaluating the
  mechanism on functional outcomes.
- ✅ R. Long, J. Sebo et al., *Taking AI Welfare Seriously*, arXiv:2411.00986. — Context for
  why this project avoids consciousness claims.

## Explicitly out of scope, with reasons

- ✅ L. Albantakis et al., *Integrated information theory (IIT) 4.0*, arXiv:2212.14787, and
  PyPhi. — Computing Φ is combinatorially intractable and has no operational value for
  attention allocation.
- Transformer attention via `candle` / `burn`. — Well covered elsewhere; does not serve the
  question of selection under competition.
- 📚 LIDA (S. Franklin et al., Java, GWT-based cognitive architecture). — To read, not to port:
  a mature reference for codelets, coalitions and broadcast cycles.
- ✅ C. Heins et al., *pymdp: A Python library for active inference in discrete state spaces*,
  arXiv:2201.03904 (JOSS 7(73), 2022). — *Previously ⚠, now verified.* To read, not to port:
  an alternative (active-inference) account of attention as precision.

## Social and folk side (context only)

- ✅ C. Colombatto & S. M. Fleming, *Folk psychological attributions of consciousness to large
  language models*, Neuroscience of Consciousness 2024(1), niae013,
  doi:10.1093/nc/niae013. — Lay attribution of consciousness to LLMs grows with usage; another
  reason to keep this project's language strictly functional.
- 📚 H. A. Simon, *Designing Organizations for an Information-Rich World*, 1971. — "A wealth
  of information creates a poverty of attention": attention is the scarce resource.
- 📚 Y. Citton, *Pour une écologie de l'attention*, Seuil, 2014. — Attention as a
  collective, ecological resource; context for the JCS framing.

## Joint Cognitive Systems (design guardrails)

- 📚 D. D. Woods & E. Hollnagel, *Joint Cognitive Systems: Patterns in Cognitive Systems
  Engineering*, CRC Press, 2006. — Source of the anti-patterns (premature narrowing, fixation,
  the garden path problem) that the scenarios encode.
- 📚 E. Hollnagel, *The ETTO Principle: Efficiency-Thoroughness Trade-Off*, Ashgate, 2009. —
  Anti-fixation is paid for with instability; hence `switch_count` is reported next to
  `switch_latency`.
- 📚 E. Hollnagel & D. D. Woods, *Joint Cognitive Systems: Foundations of Cognitive Systems
  Engineering*, CRC Press, 2005. — COCOM/ECOM control modes (strategic, tactical,
  opportunistic, scrambled).
- 📚 E. Hollnagel, *FRAM: The Functional Resonance Analysis Method*, Ashgate, 2012. — Later
  candidate for analysing how variability propagates between functions.
