//! A plain-language description of a system, for the "About this simulation" section of pages.

use crate::system::{BondType, CompartmentWalls, PistonDrive, System, Wall};
use crate::{fixed, Pair};

/// One phrase for a pair potential.
pub(crate) fn pair_phrase(p: &Pair) -> String {
    if p.eps == 0.0 || p.rc == 0.0 {
        return "no interaction".into();
    }
    let wca = (p.rc / p.sigma - 2f64.powf(1.0 / 6.0)).abs() < 1e-6;
    if wca {
        format!("repulsion only (WCA: Lennard-Jones cut at its minimum), σ = {}", num(p.sigma))
    } else {
        format!("Lennard-Jones, ε = {}, σ = {}, cut at {} σ", num(p.eps), num(p.sigma), num(p.rc / p.sigma))
    }
}

pub(crate) fn num(x: f64) -> String {
    let s = format!("{x:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

pub(crate) fn wall_phrase(w: &Wall) -> String {
    match w {
        Wall::Hard => "hard (molecules bounce off exactly)".into(),
        Wall::Soft => "soft (a smooth repulsion)".into(),
        Wall::Thermal(t) => format!("a heat bath at kT = {}", num(*t)),
    }
}

impl System {
    /// The ensemble in plain words, for the total-energy plot: what, if anything, adds or removes
    /// energy.
    pub fn energy_note(&self) -> String {
        let bath_walls = self.walls.iter().flatten().flatten().any(|w| matches!(w, Wall::Thermal(_)));
        let mut notes = vec![];
        if self.thermostat.is_some() {
            notes.push("a heat bath (constant temperature, the canonical ensemble) adds and removes energy");
        }
        if bath_walls {
            notes.push("heat-bath walls let energy in and out");
        }
        if self.piston.is_some() {
            notes.push("the piston does work on the gas");
        }
        if notes.is_empty() {
            return "Constant energy (the microcanonical ensemble): nothing adds or removes energy, so the total stays flat.".into();
        }
        let s = notes.join("; ");
        format!("{}{}.", s[..1].to_uppercase(), &s[1..])
    }

    /// Bullet points describing the method, forces, boundaries and ensemble, for `n` particles.
    /// `names` are the species' names on the page, if it has them.
    pub fn describe(&self, n: usize, names: &[String]) -> Vec<String> {
        let ns = self.species();
        let name = |s: usize| names.get(s).cloned().unwrap_or_else(|| format!("species {s}"));
        let mut out = vec![format!(
            "Method: molecular dynamics in 2D. {n} particles follow Newton's laws, integrated with the leapfrog (velocity Verlet) scheme at a time step of {} τ. Positions and velocities are exact integers (2⁻³² σ), so every run can be reversed bit for bit; forces are computed in 32-bit floats on the GPU.",
            num(self.dt)
        )];
        out.push("Units: σ is a molecule's diameter, ε the depth of the attraction, and τ = σ√(m/ε) the time unit; temperatures are given as kT in units of ε.".into());
        // Forces, grouped by potential (pairs of fixed particles do not matter).
        let mut groups: Vec<(String, Vec<String>)> = vec![];
        for a in 0..ns {
            for b in a..ns {
                if self.is_pinned(a as u32) && self.is_pinned(b as u32) {
                    continue;
                }
                let phrase = pair_phrase(self.pair(a as u32, b as u32));
                let (na, nb) = (name(a), name(b));
                let who = if na <= nb { format!("{na}–{nb}") } else { format!("{nb}–{na}") };
                match groups.iter_mut().find(|(p, _)| *p == phrase) {
                    Some((_, list)) if list.contains(&who) => {}
                    Some((_, list)) => list.push(who),
                    None => groups.push((phrase, vec![who])),
                }
            }
        }
        if groups.len() == 1 {
            out.push(format!("Forces: between every pair of molecules, {}.", groups[0].0));
        } else {
            let parts: Vec<String> = groups.iter().map(|(p, who)| format!("{p} ({})", who.join(", "))).collect();
            out.push(format!("Forces: {}.", parts.join("; ")));
        }
        let moving: Vec<usize> = (0..ns).filter(|&s| !self.is_pinned(s as u32)).collect();
        if moving.iter().any(|&s| self.masses[s] != self.masses[moving[0]]) {
            let m: Vec<String> = moving.iter().map(|&s| format!("{} {}", name(s), num(self.masses[s]))).collect();
            out.push(format!("Masses: {}.", m.join(", ")));
        }
        if !self.bonds.is_empty() {
            let kinds: Vec<String> = self
                .bond_types
                .iter()
                .map(|b| match *b {
                    BondType::Harmonic { k, r0 } if r0 == 0.0 => format!("zero-length springs (k = {})", num(k)),
                    BondType::Harmonic { k, r0 } => format!("springs (k = {}, rest length {} σ)", num(k), num(r0)),
                    BondType::Fene { k, r0 } => format!("FENE springs (k = {}, maximum length {} σ)", num(k), num(r0)),
                })
                .collect();
            out.push(format!("Bonds: {} bonded pairs; {}.", self.bonds.len(), kinds.join(", ")));
        }
        if self.pinned != 0 {
            let p: Vec<String> = (0..ns).filter(|&s| self.is_pinned(s as u32)).map(name).collect();
            out.push(format!("Fixed in place: {}.", p.join(", ")));
        }
        for (axis, both, sides) in [(0, "Left and right", ["left", "right"]), (1, "Bottom and top", ["bottom", "top"])] {
            out.push(match &self.walls[axis] {
                None => format!("{both}: periodic (a molecule leaving one side comes back on the other)."),
                Some([lo, hi]) if lo == hi => format!("{both} walls: {}.", wall_phrase(lo)),
                Some([lo, hi]) => format!("{} wall: {}; {} wall: {}.", capitalise(sides[0]), wall_phrase(lo), sides[1], wall_phrase(hi)),
            });
        }
        if self.walls.iter().flatten().flatten().any(|w| matches!(w, Wall::Thermal(_))) {
            out.push("Heat-bath walls: a molecule that hits one leaves with a fresh random velocity drawn at the wall's temperature (from the distribution of molecules crossing a surface in a gas at that temperature), whatever its speed was.".into());
        }
        if self.gravity != [0.0, 0.0] {
            out.push(format!("Gravity: a constant force m·g with g = ({}, {}) σ/τ².", num(self.gravity[0]), num(self.gravity[1])));
        }
        let c = self.compartment_counts();
        if c != [1, 1] {
            let side = |a: usize| num(fixed::to_f64(self.box_len[a]) / c[a] as f64);
            out.push(format!(
                "Boxes: {} × {} separate boxes, {} σ × {} σ; molecules cannot leave their box, and {}.",
                c[0],
                c[1],
                side(0),
                side(1),
                match self.compartment_walls {
                    CompartmentWalls::Insulating => "molecules in different boxes do not interact",
                    CompartmentWalls::Diathermal => "molecules push on their neighbours through the walls, so energy flows between boxes",
                }
            ));
        }
        if let Some(m) = &self.membrane {
            let blocked: Vec<String> = (0..ns).filter(|&s| m.blocks & (1 << s) != 0).map(name).collect();
            out.push(format!(
                "Membrane: a line across the box at {} σ that reflects {}{}.",
                num(fixed::to_f64(m.position)),
                blocked.join(" and "),
                if m.pore_shift != 0 { format!(", with pores {} σ wide every {} σ", num(fixed::to_f64(m.pore_width)), num(fixed::to_f64(1i64 << m.pore_shift))) } else { String::new() }
            ));
        }
        if let Some(p) = &self.piston {
            out.push(match p.drive {
                PistonDrive::Speed(_) => "Piston: the top wall is a piston moved at a set speed (it does work on the gas).".into(),
                PistonDrive::Free { mass, force } => format!("Piston: the top wall is a free piston of mass {} pushed down by a force {} ε/σ.", num(mass), num(force)),
            });
        }
        let bath_walls = self.walls.iter().flatten().flatten().any(|w| matches!(w, Wall::Thermal(_)));
        out.push(match &self.thermostat {
            Some(t) => {
                let who: Vec<String> = (0..ns).filter(|&s| t.species & (1 << s) != 0 && !self.is_pinned(s as u32)).map(name).collect();
                let all = who.len() == (0..ns).filter(|&s| !self.is_pinned(s as u32)).count();
                let region = t.region[0] > i64::MIN && (t.region[0] > 0 || t.region[1] < self.box_len[t.axis]);
                let axis = ["x", "y"][t.axis];
                format!(
                    "Heat bath (Andersen thermostat) at kT = {}: each molecule{} has its velocity replaced by a random one at that temperature about {} times per τ{}{}. Energy is not conserved: the bath adds and removes it.",
                    num(t.t),
                    if all { String::new() } else { format!(" of {}", who.join(" and ")) },
                    num(t.rate),
                    if region { format!(", only between {axis} = {} and {} σ", num(fixed::to_f64(t.region[0].max(0))), num(fixed::to_f64(t.region[1].min(self.box_len[t.axis])))) } else { String::new() },
                    if t.drift != 0.0 { format!(", around a mean velocity of {} σ/τ to the right (a fan)", num(t.drift)) } else { String::new() }
                )
            }
            None if bath_walls => "Ensemble: energy enters and leaves only through the heat-bath walls; otherwise it is conserved.".into(),
            None if self.piston.is_some() => "Ensemble: no heat bath; energy changes only by the work the piston does.".into(),
            None => "Ensemble: no heat bath; the total energy is conserved (constant N, V, E), apart from tiny integration errors.".into(),
        });
        out
    }
}

#[cfg(test)]
mod tests {
    use crate::{init, Pair, System, Wall};

    #[test]
    fn describes_a_simple_gas() {
        let sys = System::new(init::box_for(100, 0.5, 1.0), init::DT, Pair::wca(), vec![1.0]).with_walls(1, Wall::Hard, Wall::Hard);
        let d = sys.describe(100, &[]);
        let all = d.join("\n");
        assert!(all.contains("molecular dynamics in 2D. 100 particles"));
        assert!(all.contains("repulsion only"));
        assert!(all.contains("Left and right: periodic"));
        assert!(all.contains("Bottom and top walls: hard"));
        assert!(all.contains("total energy is conserved"));
    }
}
