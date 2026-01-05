/// Windowing utility API
use fxhash::{FxHashMap, FxHashSet};

use crate::{
    utils::{compute_levels, compute_reverse_levels, FanoutView},
    Gate, Network, Signal,
};

struct WinConfig {
    tfi_max_level: usize,
    tfo_max_level: usize,
    max_window_size: usize,
    max_fanout: usize,
}

impl Default for WinConfig {
    fn default() -> Self {
        WinConfig {
            tfi_max_level: 16,
            tfo_max_level: 16,
            max_window_size: 2000,
            max_fanout: 20,
        }
    }
}

struct Window {
    pis: Vec<Signal>,
    pos: Vec<Signal>,
    nodes: Vec<usize>,
    pivot: usize,
}

impl Window {
    pub fn new(pivot: usize) -> Self {
        Window {
            pis: Vec::new(),
            pos: Vec::new(),
            nodes: Vec::new(),
            pivot,
        }
    }
}

struct WindowConstructor<'a, 'b> {
    ntk: &'a Network,
    config: &'b WinConfig,
    levels: Vec<i32>,
    rlevels: Vec<i32>,
    max_level: i32,
    fanout_view: FanoutView,
    tfi_visited: FxHashSet<Signal>,
    pivot: Option<usize>,
}

impl<'a, 'b> WindowConstructor<'a, 'b> {
    pub fn new(ntk: &'a Network, config: &'b WinConfig) -> Self {
        let levels = compute_levels(ntk, true);
        let max_level = *levels.iter().max().unwrap_or(&0);
        let rlevels = compute_reverse_levels(ntk, true);
        let fanout_view = FanoutView::new(ntk);
        let tfi_visited = FxHashSet::default();
        WindowConstructor {
            ntk,
            config,
            levels,
            rlevels,
            max_level,
            fanout_view,
            tfi_visited,
            pivot: None,
        }
    }

    pub fn set_pivot(&mut self, pivot: usize) {
        self.reset();
        self.pivot = Some(pivot);
    }

    pub fn reset(&mut self) {
        self.tfi_visited = FxHashSet::default();
        self.pivot = None;
    }

    pub fn construct(&mut self, pivot: usize) -> Option<Window> {
        self.pivot = Some(pivot);
        todo!()
    }

    fn collect_tfi_cone(&mut self) -> Vec<Signal> {
        fn tfi_rec(
            node: usize,
            ntk: &Network,
            visited: &mut FxHashSet<Signal>,
            limit: usize,
            nodes: &mut Vec<Signal>,
        ) -> bool // return true if window size exceeds limit
        {
            let sig = ntk.node(node).without_inversion();
            if visited.contains(&sig) {
                return false;
            }
            visited.insert(sig);

            let g = ntk.gate(node);
            for fanin in g.dependencies().iter() {
                if fanin.is_input() {
                    let pi = fanin.without_inversion();
                    if !visited.contains(&pi) {
                        nodes.push(pi);
                        visited.insert(pi);
                    }
                } else if fanin.is_constant() {
                    //0 and 1 are allowed to be added to the nodes vector
                    if !visited.contains(fanin) {
                        nodes.push(*fanin);
                        visited.insert(*fanin);
                    }
                } else if tfi_rec(fanin.var() as usize, ntk, visited, limit, nodes) {
                    return true;
                }
            }

            nodes.push(sig);
            return nodes.len() > limit;
        }

        let mut nodes = Vec::new();
        assert!(
            self.pivot.is_some(),
            "Pivot must be set before collecting TFI cone"
        );
        let limit = self.config.max_window_size;
        if tfi_rec(
            self.pivot.unwrap(),
            self.ntk,
            &mut self.tfi_visited,
            limit,
            &mut nodes,
        ) {
            // exceed limit
            Vec::new()
        } else {
            nodes
        }
    }

    fn node_tfo_cone(&self, pivot: usize) -> Vec<Signal> {
        todo!()
    }

    fn add_divisor(
        &self,
        fanin_counts: &mut FxHashMap<Signal, usize>,
        divisors: &mut Vec<Signal>,
        node: Signal,
        level_max: i32,
    ) {
        let fanouts = self.fanout_view.fanouts(node);
        if fanouts.len() > self.config.max_fanout {
            return;
        }
        for fanout in fanouts.iter() {
            let fanout = *fanout as usize;

            // skip if node is already in TFI or level exceeds limit
            if self
                .tfi_visited
                .contains(&self.ntk.node(fanout).without_inversion())
                || self.levels[fanout] > level_max
            {
                continue;
            }

            let fanin_count = self.ntk.gate(fanout).dependencies().len();
            // handle single-input nodes
            if fanin_count == 1 {
                divisors.push(self.ntk.node(fanout));
                continue;
            }

            let remaining = fanin_counts
                .entry(self.ntk.node(fanout).without_inversion())
                .or_insert(fanin_count);

            if *remaining != 0 {
                *remaining -= 1;
                divisors.push(self.ntk.node(fanout));
            }
        }
    }

    fn collect_divisors(&self, tfi_cone: &[Signal]) -> Vec<Signal> {
        let mut divisors = tfi_cone.to_vec();
        let pivot_signal = self.ntk.node(self.pivot.unwrap()).without_inversion();
        // pop out pivot
        assert_eq!(
            Some(pivot_signal),
            divisors.pop().map(|s| s.without_inversion())
        );

        let mut fanout_count = FxHashMap::default();
        for node in tfi_cone {
            let level_max = if !node.is_var() && !node.is_constant() {
                self.max_level
            } else {
                self.max_level - self.rlevels[node.var() as usize]
            };
            self.add_divisor(&mut fanout_count, &mut divisors, *node, level_max);
        }

        println!(
            "pivot: {}, before compacting divisors: {:?}",
            self.pivot.unwrap(),
            divisors
        );

        // Remove POs, pivot and its fanins to compact divisors
        let mut useless = FxHashSet::default();
        useless.insert(pivot_signal);
        let pivot = self.pivot.unwrap();
        for fanin in self.ntk.gate(pivot).dependencies() {
            useless.insert(fanin.without_inversion());
        }
        for po in 0..self.ntk.nb_outputs() {
            useless.insert(self.ntk.output(po).without_inversion());
        }
        divisors.retain(|s| !useless.contains(&s.without_inversion()));

        if divisors.len() > self.config.max_window_size {
            divisors.truncate(self.config.max_window_size);
        }
        divisors
    }
}

// pub fn node_window(aig: &Network, node: usize, config: &WinConfig) -> Window {
//     node_window(aig, node, &config)
// }

#[cfg(test)]
mod tests {
    use super::*;

    fn validate_signals(
        signals: Vec<Signal>,
        expected_len: usize,
        name: &str, // for error messages: "tfi_cone" or "divisors"
    ) -> Result<Vec<Signal>, String> {
        let set = signals.iter().collect::<FxHashSet<_>>();
        if set.len() != signals.len() {
            return Err(format!(
                "{name} has duplicates: set.len()={} != len()={}",
                set.len(),
                signals.len()
            ));
        }
        if signals.len() != expected_len {
            return Err(format!(
                "{name} length mismatch: len()={} != expected={}",
                signals.len(),
                expected_len
            ));
        }
        Ok(signals)
    }

    fn run_collect_tfi(
        constructor: &mut WindowConstructor,
        pivot: usize,
        expected_len: usize,
    ) -> Result<Vec<Signal>, String> {
        constructor.set_pivot(pivot);
        let tfi_cone = constructor.collect_tfi_cone();
        println!("pivot: {pivot}, tfi_cone: {:?}", tfi_cone);
        validate_signals(tfi_cone, expected_len, "tfi_cone")
    }

    fn run_collect_divisors(
        constructor: &WindowConstructor,
        tfi_cone: &[Signal],
        expected_len: usize,
    ) -> Result<Vec<Signal>, String> {
        let divisors = constructor.collect_divisors(tfi_cone);
        println!("      divisors: {:?}", divisors);
        validate_signals(divisors, expected_len, "divisors")
    }

    #[test]
    fn test_window_construction() {
        let mut aig = Network::default();
        let x1 = aig.add_input();
        let x2 = aig.add_input();
        let x3 = aig.add_input();
        let x4 = aig.add_input();

        let f1 = aig.and(x1, x2);
        let f2 = aig.and(x3, x4);
        let f3 = aig.and(x1, x3);
        let f4 = aig.and(f1, f2);
        let f5 = aig.and(f3, f4);

        aig.add_output(f3);
        aig.add_output(!f5);

        // println!("{}", aig);
        // crate::io::write_dot_file(&std::path::PathBuf::from("window.dot"), &aig);

        let config = WinConfig::default();
        let mut constructor = WindowConstructor::new(&aig, &config);

        run_collect_tfi(&mut constructor, 4, 9)
            .and_then(|tfi_cone| run_collect_divisors(&constructor, &tfi_cone, 6))
            .unwrap();
        run_collect_tfi(&mut constructor, 3, 7)
            .and_then(|tfi_cone| run_collect_divisors(&constructor, &tfi_cone, 4))
            .unwrap();
        run_collect_tfi(&mut constructor, 2, 3)
            .and_then(|tfi_cone| run_collect_divisors(&constructor, &tfi_cone, 2))
            .unwrap();
        run_collect_tfi(&mut constructor, 1, 3)
            .and_then(|tfi_cone| {
                run_collect_divisors(
                    &constructor,
                    &tfi_cone,
                    /* remove n2 because it's a PO*/ 0,
                )
            })
            .unwrap();
        run_collect_tfi(&mut constructor, 0, 3)
            .and_then(|tfi_cone| run_collect_divisors(&constructor, &tfi_cone, 0))
            .unwrap();
    }
}
