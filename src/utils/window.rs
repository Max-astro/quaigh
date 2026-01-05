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
    tfi_visited: FxHashSet<usize>,
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
        }
    }

    pub fn construct(&self, node: usize) -> Option<Window> {
        todo!()
    }

    fn collect_tfi_cone(&mut self, node: usize, win: &mut Window) {
        fn tfi_rec(
            node: usize,
            ntk: &Network,
            visited: &mut FxHashSet<usize>,
            limit: usize,
            nodes: &mut Vec<usize>,
        ) -> bool {
            if visited.contains(&node) {
                return false;
            }
            visited.insert(node);

            let g = ntk.gate(node);
            for fanin in g.dependencies().iter().filter(|s| s.is_var()) {
                if tfi_rec(fanin.var() as usize, ntk, visited, limit, nodes) {
                    return true;
                }
            }

            nodes.push(node);
            return nodes.len() > limit;
        }

        let limit = self.config.max_window_size;
        tfi_rec(node, self.ntk, &mut self.tfi_visited, limit, &mut win.nodes);
    }

    fn node_tfo_cone(&self, node: usize) -> Vec<Signal> {
        todo!()
    }

    fn add_divisor(
        &self,
        fanout_count: &mut FxHashMap<usize, usize>,
        divisors: &mut Vec<usize>,
        node: usize,
        level_max: i32,
    ) {
        let fanouts = self.fanout_view.fanouts(self.ntk.node(node));
        if fanouts.len() > self.config.max_fanout {
            return;
        }
        for fanout in fanouts.iter().filter(|&&n| {
            let n = n as usize;
            !(self.tfi_visited.contains(&n) || self.levels[n] > level_max)
        }) {
            let fanout = *fanout as usize;
            let fanin_count = self.ntk.gate(fanout).dependencies().len();
            // handle single-input nodes
            if fanin_count == 1 {
                divisors.push(fanout);
                continue;
            }

            let remaining = fanout_count.entry(fanout).or_insert(fanin_count);
            if *remaining != 0 {
                *remaining -= 1;
                divisors.push(fanout);
            }
        }
    }

    fn collect_divisors(&self, tfi_cone: &[usize]) -> Vec<usize> {
        let mut divisors = tfi_cone.to_vec();
        divisors.pop(); // pop out pivot

        let mut fanout_count = FxHashMap::default();
        for node in tfi_cone[..tfi_cone.len() - 1].iter() {
            self.add_divisor(
                &mut fanout_count,
                &mut divisors,
                *node,
                self.max_level - self.rlevels[*node],
            );
        }
        
        if divisors.len() > self.config.max_window_size {
            divisors.truncate(self.config.max_window_size);
        }
        divisors
    }
}

// pub fn node_window(aig: &Network, node: usize, config: &WinConfig) -> Window {
//     node_window(aig, node, &config)
// }
