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
    max_tfo_level: usize,
}

impl Default for WinConfig {
    fn default() -> Self {
        WinConfig {
            tfi_max_level: 16,
            tfo_max_level: 16,
            max_window_size: 2000,
            max_fanout: 20,
            max_tfo_level: 20,
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

struct WindowConstructor<'a> {
    ntk: &'a Network,
    config: &'a WinConfig,
    levels: Vec<i32>,
    rlevels: Vec<i32>,
    max_level: i32,
    fanout_view: FanoutView,
    tfi_visited: FxHashSet<Signal>,
    pos: FxHashSet<Signal>,
    pivot: Option<usize>,
}

impl<'a> WindowConstructor<'a> {
    pub fn new(ntk: &'a Network, config: &'a WinConfig) -> Self {
        let levels = compute_levels(ntk, true);
        let max_level = *levels.iter().max().unwrap_or(&0);
        let rlevels = compute_reverse_levels(ntk, true);
        let fanout_view = FanoutView::new(ntk);
        let tfi_visited = FxHashSet::default();
        let pos = (0..ntk.nb_outputs())
            .map(|po| ntk.output(po).without_inversion())
            .collect();

        WindowConstructor {
            ntk,
            config,
            levels,
            rlevels,
            max_level,
            fanout_view,
            tfi_visited,
            pos,
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

    fn tfi_rec(
        &self,
        tfi_visited: &mut FxHashSet<Signal>,
        node: usize,
        nodes: &mut Vec<Signal>,
        limit: usize,
    ) -> bool // return true if window size exceeds limit
    {
        let sig = self.ntk.node(node).without_inversion();
        if tfi_visited.contains(&sig) {
            return false;
        }
        tfi_visited.insert(sig);

        let g = self.ntk.gate(node);
        for fanin in g.dependencies().iter() {
            if fanin.is_input() {
                let pi = fanin.without_inversion();
                if !tfi_visited.contains(&pi) {
                    nodes.push(pi);
                    tfi_visited.insert(pi);
                }
            } else if fanin.is_constant() {
                //0 and 1 are allowed to be added to the nodes vector
                if !tfi_visited.contains(fanin) {
                    nodes.push(*fanin);
                    tfi_visited.insert(*fanin);
                }
            } else if self.tfi_rec(tfi_visited, fanin.var() as usize, nodes, limit) {
                return true;
            }
        }

        nodes.push(sig);
        return nodes.len() > limit;
    }

    fn collect_tfi_cone(&mut self) -> Option<Vec<Signal>> {
        let mut nodes = Vec::new();
        assert!(
            self.pivot.is_some(),
            "Pivot must be set before collecting TFI cone"
        );
        let mut tfi_visited = FxHashSet::default();
        if self.tfi_rec(
            &mut tfi_visited,
            self.pivot.unwrap(),
            &mut nodes,
            self.config.max_window_size,
        ) {
            // exceed limit
            None
        } else {
            self.tfi_visited = tfi_visited;
            Some(nodes)
        }
    }

    fn is_root_node(&self, node: usize, max_level: i32) -> bool {
        let fanouts = self.fanout_view.fanouts(self.ntk.node(node));
        if fanouts.is_empty() || fanouts.len() > self.config.max_fanout {
            return true;
        }

        // PO should be considered as root node
        let signal = self.ntk.node(node).without_inversion();
        if self.pos.contains(&signal) {
            return true;
        }

        for fanout in fanouts {
            let n = *fanout as usize;
            let lv = self.levels[n];
            if lv > max_level {
                return true;
            }
        }

        false
    }

    fn tfo_rec(
        &self,
        node: usize,
        lv: i32,
        pivot: usize,
        visited: &mut FxHashSet<Signal>,
        roots: &mut Vec<usize>,
        tfo: &mut Vec<usize>,
    ) {
        let sig = self.ntk.node(node).without_inversion();
        if visited.contains(&sig) {
            return;
        }
        visited.insert(sig);

        if node != pivot {
            tfo.push(node);
        }

        if self.is_root_node(node, lv) {
            roots.push(node);
        } else {
            for fanout in self.fanout_view.fanouts(sig) {
                let fanout = *fanout as usize;
                self.tfo_rec(fanout, lv, pivot, visited, roots, tfo);
            }
        }
    }

    // return (roots, tfo)
    fn collect_root_and_tfo(&self) -> (Vec<usize>, Vec<usize>) {
        let mut roots = Vec::new();
        let mut tfo = Vec::new();
        let mut visited = FxHashSet::default();

        let node = self.pivot.unwrap();
        let lv = self.levels[node] + self.config.max_tfo_level as i32;
        self.tfo_rec(node, lv, node, &mut visited, &mut roots, &mut tfo);

        (roots, tfo)
    }

    fn traverse_tfi(
        &self,
        visited: &mut FxHashSet<Signal>,
        mut topo_ord: Vec<Signal>,
        limit: usize,
        check_nodes: impl IntoIterator<Item = usize>,
    ) -> Option<Vec<Signal>> {
        for n in check_nodes {
            if self.tfi_rec(visited, n, &mut topo_ord, limit) {
                return None;
            }
        }
        Some(topo_ord)
    }

    fn closuring_window(
        &self,
        roots: &[usize],
        tfo: &[usize],
        divisors: &[Signal],
    ) -> Option<Vec<Signal>> {
        let topo_ord = self.traverse_tfi(
            &mut FxHashSet::default(),
            Vec::new(),
            self.config.max_window_size,
            roots
                .iter()
                .chain(tfo.iter())
                .copied()
                .chain(divisors.iter().filter_map(|s| {
                    if s.is_var() {
                        Some(s.var() as usize)
                    } else {
                        None
                    }
                })),
        );

        let pivot = self.pivot.unwrap();
        // fall back if topo_ord exceeds limit
        topo_ord.or(self.traverse_tfi(
            &mut FxHashSet::default(),
            Vec::new(),
            usize::MAX,
            vec![pivot]
                .iter()
                .copied()
                .chain(divisors.iter().filter_map(|s| {
                    if s.is_var() {
                        Some(s.var() as usize)
                    } else {
                        None
                    }
                })),
        ))
    }

    fn add_divisor(
        &self,
        fanin_counts: &mut FxHashMap<Signal, usize>,
        divisors: &mut Vec<Signal>,
        node: Signal,
        level_max: i32,
    ) {
        let fanouts = self.fanout_view.fanouts(node);
        // skip PO or fanout count exceeds limit
        if fanouts.is_empty() || fanouts.len() > self.config.max_fanout {
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

        let mut fanin_counts = FxHashMap::default();
        for node in tfi_cone {
            let level_max = if !node.is_var() {
                self.max_level
            } else {
                self.max_level - self.rlevels[node.var() as usize]
            };
            self.add_divisor(&mut fanin_counts, &mut divisors, *node, level_max);
        }

        // println!(
        //     "pivot: {}, before compacting divisors: {:?}",
        //     self.pivot.unwrap(),
        //     divisors
        // );

        // Remove POs, pivot and its fanins to compact divisors
        let mut useless = FxHashSet::default();
        useless.insert(pivot_signal);
        let pivot = self.pivot.unwrap();
        for fanin in self.ntk.gate(pivot).dependencies() {
            useless.insert(fanin.without_inversion());
        }

        divisors.retain(|s| {
            !(useless.contains(&s.without_inversion()) || self.pos.contains(&s.without_inversion()))
        });

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
        // println!("pivot: {pivot}, tfi_cone: {:?}", tfi_cone);
        tfi_cone.map_or(Err(format!("TFI cone is empty")), |tfi_cone| {
            validate_signals(tfi_cone, expected_len, "tfi_cone")
        })
    }

    fn run_collect_divisors(
        constructor: &WindowConstructor,
        tfi_cone: &[Signal],
        expected_len: usize,
    ) -> Result<Vec<Signal>, String> {
        let divisors = constructor.collect_divisors(tfi_cone);
        // println!("      divisors: {:?}", divisors);
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

    #[test]
    fn test_closuring_window() {
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
        let f6 = aig.add(Gate::mux(f4, Signal::one(), f5));

        aig.add_output(f3);
        aig.add_output(!f5);
        aig.add_output(f6);

        // println!("{}", aig);
        // crate::io::write_dot_file(&std::path::PathBuf::from("window2.dot"), &aig);

        let config = WinConfig::default();
        let mut constructor = WindowConstructor::new(&aig, &config);

        let mut traverse_fanout = |node: usize,
                                   expected_window_size: usize,
                                   expected_roots: &[usize],
                                   expected_tfo: &[usize]| {
            constructor.set_pivot(node);
            // println!("pivot: {}", node);
            let tfi_cone = constructor.collect_tfi_cone();
            let divisors = tfi_cone
                .as_ref()
                .map(|tfi_cone| constructor.collect_divisors(tfi_cone));
            // println!("divisors: {:?}", divisors);

            let (roots, tfo) = constructor.collect_root_and_tfo();
            // println!("roots: {:?}", roots);
            // println!("tfo: {:?}", tfo);
            assert_eq!(roots, expected_roots);
            assert_eq!(tfo, expected_tfo);

            let window = constructor.closuring_window(&roots, &tfo, divisors.as_ref().unwrap());
            // println!("window: {:?}\n", window);

            assert_eq!(window.as_ref().unwrap().len(), expected_window_size);
        };

        traverse_fanout(5, 11, &[5], &[]);
        traverse_fanout(4, 9, &[4], &[]);
        traverse_fanout(3, 11, &[4, 5], &[4, 5]);
        traverse_fanout(2, 7, &[2], &[]);
        traverse_fanout(1, 11, &[4, 5], &[3, 4, 5]);
        traverse_fanout(0, 11, &[4, 5], &[3, 4, 5]);
    }
}
