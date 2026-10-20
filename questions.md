
Why does the boiling point rise with pressure?

Why does mixing salt into water lower its melting point?

Why does air pressure fall with height?

Why is it colder at high altitude?

Why do cumulus clouds have flat bases?

What’s so special about the _Gibbs_ free energy? How come it’s always showing up in chemistry and biology?

Why do we sometimes talk about enthalpy and other times about energy or heat?

How come there’s a triple point of water? What about in real life, if there’s some ice in my drink,  if there’s an iceberg in the ocean, doesn’t the air above it also have some water vapor? Is this happening at the triple point? Probably not, right? The pressure is just the usual 1atm? Is that because these systems are not in equilibrium?

How comes the Boltzmann distribution just shows lower energy states always more likely, but Maxwell’s distribution shows a specific nonzero speed as being the likeliest?

Why is osmosis an entropic force? Where does the force, the actual push, come from in terms of the particles of solvent and solute?

Explain in detail why temperature doesn’t rise during melting or vaporization.

Why is dQ = dS/T? Why is the sneeze-in-library metaphor true?

How is the Boltzmann distribution derived?

How do buffer solutions work, in particular the blood? Simulate using particles of proton, the weak-binding negative ion (strong acid), the strong-binding negative ion (weak acid).

What’s the average kinetic energy of a molecule of an ideal gas?

Why does cooking at a higher temperature cook the food faster? For example beans going from dry to soaked and softened; chicken cooked; potatoes cooked. Answers in Schroeder 1.7, 5.60 kinetics lectures, Nelson 9.5, McQuarrie chapter 28, 

Why is entropy as a function of energy a monotonically increasing function?

What does salt dissolve in water? Why does solubility increase with temperature? If water already has salt dissolved, does that affect how much sugar can be dissolved? Why does salt change the melting point so much but not the boiling point of water as much? Answer: dissolving is a free energy gain mainly because of translational entropy gain. Schroeder 5.6 and McQuarrie chapter 26. Le Chatelier and van’t Hoff’s equation. 5.60 L21-22. McQuarrie 24 and 26.

Why does air hold more water at higher temperature? What does saturation by water vapor mean, and why does that happen? Feynman vol 1 chapter 42. Schroeder 5.3 Clausiu-Chapeyron, 5.60 L18-19, 3.020 L12

Why is the Carnot engine the most efficient possible?

Does osmosis need attraction between solute and solvent particles? Is it caused by dilution of solvent particles by the solute?

Einstein's annus mirabilis papers

How do lipid bilayers self-assemble? Can we show this in a lattice model? Is it driven by entropic effects or energetic or both? What happens if it's too hot? What happens if the environment is too acidic?

Why are many injections or IV drips done using saline solution? What would go wrong if it was pure water? What if it was much saltier?

How do mammalian kidneys manage to concentrate urine to even more than blood concentration?

# More questions (brainstorm, October 2026)

A brainstorm of further questions the engine could address, with my verdict on each. Engine tags in brackets refer to `docs/architecture.md`. "Notes" are follow-up thoughts on feasibility.

## Basic statistical mechanics

Why can't you un-stir cream from coffee, when every collision can run backwards? [md, exact reversal]
- **Verdict:** yes. Run forward, flip every velocity, and it un-mixes exactly; flip one bit first and it never does. Also want to show that at low Reynolds number you CAN unstir (G. I. Taylor's glycerol demo).
- Notes: these are two different kinds of reversibility. MD reversal means reversing every molecular velocity. Taylor's demo reverses only the crank; it works because Stokes flow is linear and the dye diffuses slowly (high Péclet number). In nanoscale MD the fluid is low-Re, but diffusion is fast, so the dye blurs. The low-Re page is probably best as a Stokes/Couette tracer sim with a diffusion (Péclet) knob. We could also try it in MD with large, heavy, slow-diffusing tracers.

Could all the air in your room rush into one corner? How long would you wait? [md]
- **Verdict:** yes. Interactively, show people how often it happens: count the events live for small N and extrapolate the 2^-N scaling to a room.

Why is one molecule's energy distributed exponentially when the whole gas has a fixed total? [md]
- **Verdict:** yes. Tag one particle among a million and histogram its energy; grow the tagged group and watch the exponential become a narrow Gaussian. This is the Boltzmann derivation from a big isolated ensemble.

At the same temperature, does a heavy molecule have less energy than a light one? [md]
- **Verdict:** yes. Not just helium vs xenon: also real big boys, e.g., a really big heavy particle in the gas.
- Notes: a big heavy particle that still gets 3/2 kT is Brownian motion, so this ties into the Einstein 1905 question. Mass ratios of 10^3–10^4 are fine; the timestep is set by the light particles.

What is the depletion force (the sheep and sheepdog effect)? [md or hpmc]
- **Verdict:** yes, add it.
- Notes: two big disks in a bath of small ones are pushed together purely entropically (Asakura–Oosawa). Measure the effective force between them and compare to the AO formula. This ties to crowding in cells and colloids clumping.

Why do Brazil nuts end up on top of the mixed-nuts can, and where does that effect show up and where doesn't it? [md + granular extension]
- **Verdict:** yes. Show where the Brazil nut effect appears and where it doesn't; it might not show up in ordinary MD.
- Notes: with elastic collisions in thermal equilibrium under gravity, each species follows its own barometric law, so a big dense particle sinks: no Brazil nut effect. With inelastic collisions plus a shaking floor (driven granular matter, far from equilibrium), it rises. The same particles and gravity with one switch flipped makes a good contrast. This needs a dissipative (spring-dashpot) pair force and a vibrating wall.

Can heat ever flow from cold to hot? [md, small N]
- **Verdict:** yes. Show heat occasionally flowing from cold to hot. The fluctuation theorem gives how often.

Can a trapdoor that only lets fast molecules through make a free refrigerator (Maxwell's demon)? [md]
- **Verdict:** yes. Feynman has a detailed argument (ratchet and pawl, Feynman Lectures Vol. I, Ch. 46: https://www.feynmanlectures.caltech.edu/I_46.html). Link to it and show the problem happening in action: at uniform temperature the trapdoor heats up and jiggles open as often as it lets molecules through.

Why does a stretched rubber band feel warm, and why does it shrink when heated? [md]
- **Verdict:** yes. Show the rubber band as an entropic force.

Why don't oil and water mix? [md]
- **Verdict:** yes. Show that water and oil particles are actually weakly ATTRACTIVE, and entropy favors mixing, yet they still separate. Interactively, let people tune the relative attractions (water–water, oil–oil, water–oil) as well as temperature.

Entropic effects in general (cross-cutting)
- **Verdict:** for entropically driven effects, emphasize how they depend on temperature.
- Notes: the clean signature is that an entropic force gets STRONGER in proportion to T at fixed configuration (F = T dS/dx). Examples: rubber band tension, osmotic pressure, depletion force. Everything moves faster when hot, so "faster" alone doesn't distinguish entropic from energetic effects; "stronger when hotter" does. The rubber band shrinking when heated is the showcase.

## Physical chemistry

Why does a bottle of supercooled water freeze instantly when you tap it? [md]
- **Verdict:** yes, nucleation, do it.

Why does rain need dust, and can cloud seeding work? [md]
- **Verdict:** unsure if it can be done in MD, but try.
- Notes: heterogeneous vs homogeneous nucleation of vapor on a seed particle is doable in LJ. MD needs much higher supersaturation than real clouds, so it's qualitative. Whether seeding works in practice is debated (Delhi smog trials, UAE).

Why do bubbles form at the bottom of the pot, and does salt make water boil faster? [md]
- **Verdict:** unsure if boiling nucleation can be done interactively in MD, but try.
- Notes: bubble nucleation at a superheated wall is done in the nanoscale MD literature. Use a thermal wall above the boiling point in 2D.

Why does ice float, why do lakes freeze from the top, and why do pipes burst? [mW water]
- **Verdict:** yes, explore water freezing and its density curve.

Is pH 7 always neutral? How much does water split into ions? [kinetics]
- **Verdict:** yes, explore water splitting into ions. This ties into the buffer question.
- Notes: classical water models can't break bonds, so this is a reaction/kinetics model, not plain MD.

How does a catalyst speed up a reaction? The ammonia (Haber–Bosch) process, simplified. [md with toy reactive potential, or lattice kinetic MC]
- **Verdict:** yes, explore chemical kinetics with a catalyst, using a simpler case: a simplified version of ammonia synthesis catalysis.
- Notes: N2 splitting is the slow step and the surface lowers its barrier. There's a tradeoff: low T favors ammonia at equilibrium but is too slow, high pressure favors it (4 gas molecules become 2). Hence ~400–450 °C and 150–300 atm. This ties to the pressure pages and Le Chatelier.

## Everyday life

Pressure cookers [md]
- **Verdict:** for sure, a bunch of pages, including tying into the pressure vs height page.
- Notes, possible pages:
  - the weight on the whistle/valve sets the pressure;
  - pressure sets the boiling point (~120 °C), tying to "why does the boiling point rise with pressure";
  - dal doesn't cook in Leh because water boils near 87 °C there, tying to "why does air pressure fall with height";
  - food cooks faster at 120 °C (Arrhenius), tying to the existing cooking question and to reaction rates below;
  - opening one early is dangerous because the superheated water flash-boils.

Evaporative cooling: how does sweating cool you? [md]
- **Verdict:** yes, do evaporative cooling to show how sweating works (instead of the matka clay-pot page).

Why is humid heat deadlier than dry heat? [md]
- **Verdict:** yes, extend the evaporative cooling page to show the humidity effect on sweating (wet-bulb limit, heatwaves in India and the US).

Why does being sweaty in the wind feel cold? [md + driven "fan" region]
- **Verdict:** yes (instead of blowing on chai). Have a driven, fan-type gadget in the sim.
- Notes: the fan can be a region that adds drift momentum to gas particles and sweeps vapor away from the liquid surface, with a renewal boundary downstream.

How can a heat pump deliver more heat than the electricity it uses, and why do ACs struggle in a heat wave? [md]
- **Verdict:** yes, heat pumps are great for this.

Why are engines only ~30% efficient? [md, finite-time Carnot]
- **Verdict:** yes, explore heat engine efficiency. Not sure it can be done in MD, but worth a shot.
- Notes: this is M2 in the architecture (piston, thermal walls, heat ledger; validate against Izumida–Okuda).

Why does fast compression heat a gas and fast expansion cool it? Bike pumps, spray cans. [md, piston]
- **Verdict:** yes, the bike pump and spray can examples are good.
- Notes: the bike pump is pure compression heating. A spray can's cold is mostly evaporation of the liquid propellant, so the page should say so (or that becomes a second mechanism to show).

Why does a helium balloon go flat faster than an air-filled one? [md, porous membrane]
- **Verdict:** yes, cool, reasonably doable in MD.
- Notes: in real latex, gas dissolves into and diffuses through the rubber rather than escaping through pores, but the light-escapes-faster trend is the same. A pore membrane with two masses shows Graham's law directly.

Why does warm soda go flat, and why do fish suffocate in warm water? [md]
- **Verdict:** yes, explore gas solvation and its temperature dependence.

Why does road salt stop working below about −21 °C? [md]
- **Verdict:** yes, road salt and its temperature dependence.
- Notes: the eutectic is qualitative with model particles.

How long would a smell take to cross a room by diffusion alone, vs with air currents? [md]
- **Verdict:** yes, diffusion vs air currents.
- Notes: correction to the brainstorm, which said "hours". For D ≈ 0.1 cm²/s, crossing ~5 m by diffusion alone takes ~L²/2D ≈ 10^6 s, roughly two weeks. Air currents do essentially all the work.

Why does smog get trapped over Delhi and LA in winter? [md, gravity column]
- **Verdict:** yes, explore smog. Delhi might have a bowl-shaped geometry too?
- Notes: LA is the textbook basin ringed by mountains. Delhi sits in the Indo-Gangetic plain, hemmed in by the Himalayas to the north. Winter inversions plus low wind trap the smoke (including crop burning). "Bowl" is sometimes used loosely for Delhi; check before claiming it.

Why do mountains cast rain shadows? [probably not MD]
- **Verdict:** explore it; unclear whether MD or some other kind of sim.
- Notes: the ingredients (adiabatic cooling of rising air, condensation, flat cloud bases) are MD-friendly and tie to the cumulus question. The rain shadow itself is better as an air-parcel model following air over a mountain profile with dry and moist lapse rates. Examples: Western Ghats and the Deccan, Sierra Nevada and Death Valley, Himalaya and Tibet.

Why does a hot-air balloon rise? Is a rising pocket of less-dense air essentially an entropic effect? [md, gravity]
- **Verdict:** yes, for sure.
- Notes: partly. The push is ordinary gas pressure, which for an ideal gas is the same kinetic/entropic pressure as in osmosis. The energy that drives the motion is gravitational: the colder, denser air around the balloon sinks and displaces the warm pocket upward. So it's more buoyancy than entropy. The page can show both views.

## Biology and health

Why does a cooked egg turn white and solid, and why can't you uncook it? [lattice protein model]
- **Verdict:** yes, if protein unfolding can be shown in a browser sim. Also show the effect of pH or salt on protein folding.
- Notes: charged residues in an HP-style lattice model. pH sets which residues carry charge; salt screens the charges.

How does a protein find its folded shape (Levinthal's paradox)? [lattice protein model]
- **Verdict:** yes, protein folding in a sim if possible.

Why is it hard to breathe on Everest or in Ladakh? [md barometric; binding model]
- **Verdict:** oxygen partial pressure falling with height is one little demo. Unsure the hemoglobin binding curve can be done in MD.
- Notes: the S-shaped curve would be a cooperative binding model (4 sites, MWC/Hill) in the kinetics/lattice engine, not MD.

How does carbon monoxide poisoning work? [kinetics/binding]
- **Verdict:** great example. Find a way to show it in a sim.
- Notes: competitive binding. CO wins by ~200× because a small binding-energy difference becomes a big Boltzmann factor.

How do molecular motors walk in a storm of thermal noise? [Langevin, 1D]
- **Verdict:** amazing if doable in a browser, but skeptical; maybe something mega simplified.
- Notes: the mega-simplified version is a flashing ratchet: a Brownian particle in a sawtooth potential switched on and off by "fuel". That's very cheap, and it pairs with the Feynman ratchet page.

How do trees pull water up 100 m? Can liquid water be under tension, and what happens when it cavitates? [md]
- **Verdict:** can we show this with MD?
- Notes: yes, qualitatively. Stretch an LJ liquid to negative pressure and watch it cavitate. Real water's tensile strength needs a water model (mW). Xylem runs at roughly −1 to −2 MPa.

How does a COVID PCR test copy DNA? [coarse bead model]
- **Verdict:** yes, a simplified sim of COVID PCR would be cool.

## COVID, vaccines, public health

How can a mask stop particles much smaller than its holes? [Langevin particles past fibers]
- **Verdict:** yes, for sure.

Why does soap destroy the coronavirus envelope? [lattice/md]
- **Verdict:** yes, a model of soap would be great; might be similar to the lipid bilayer question.

Why does dry indoor winter air help respiratory viruses spread? [md]
- **Verdict:** unsure if doable in browser MD, but worth a shot.
- Notes: MD can show a nanodroplet evaporating faster into drier vapor. Real droplets are micrometers across and the epidemiology is out of scope.

Why do reaction rates roughly double every 10 °C? Insulin, vaccines, cooking. [kinetics]
- **Verdict:** yes. Talk about insulin storage in heat and cooking, which connects to how pressure cookers work. (mRNA vaccine cold chain too.)

## Drug discovery

Why does a drug stick to its target and not to everything else? [md]
- **Verdict:** a drug binding model would be amazing and a perfect MD demo, if it can be made fast enough.
- Notes: 2D pocket + ligand + solvent; binding constant from occupancy counting.

Why can a tiny change to a drug molecule change its potency a lot? [md]
- **Verdict:** yes, drug discovery exploration would be great.
- Notes: at body temperature, 1.4 kcal/mol is a factor of 10.

How do computers predict which drug candidate binds best? Alchemical free energy (FEP). [md]
- **Verdict:** AMAZING. I worked on FEP calculations at DESRES, so doing this interactively on the GPU in the browser would be really sick.
- Notes: λ windows map naturally onto batched replicas (one dispatch, all windows). Soft-core potentials; TI, BAR and MBAR estimators. Relative binding via the thermodynamic cycle. Validate against direct-counting binding constants from the binding page.

## Skipped

- How do insects walk on water / measuring surface tension: measuring surface tension doesn't connect directly to the real question.
- Is glass a liquid? Glass in MD isn't doable; it's too slow for anything to happen. That's the point, but everything in MD is too slow.
- Why do nanoparticles melt at lower temperatures? Skip.
- Why is ice slippery? Skip.
- How does a matka (clay pot) cool water? Replaced by the evaporative cooling / sweating pages.
- Why does blowing on chai cool it? Replaced by sweaty in the wind with a fan gadget.
- How do Arctic fish keep from freezing (antifreeze proteins)? Skip.
- Why do some drugs reach the brain (blood–brain barrier, logP)? Skip.
- The ritonavir polymorph story: too complex.
