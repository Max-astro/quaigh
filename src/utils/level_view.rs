use fxhash::FxHashSet;

use crate::utils::FanoutView;
use crate::{Gate, Network, Signal};

struct LevelViewBuilder<'a> {
    ntk: &'a Network,
    pi_levels: Option<&'a Vec<u32>>,
    count_buffer: bool,
    visited: FxHashSet<u32>,
}

impl<'a> LevelViewBuilder<'a> {
    /// Compute the levels of the network
    ///
    /// # Arguments
    /// * `ntk` - The network to compute the levels of
    /// * `pi_levels_config` - The levels of the primary inputs
    /// * `count_buffer` - Whether to count buffers as levels
    ///
    /// # Returns
    /// A vector of levels for each node in the network
    pub fn compute_levels(
        ntk: &'a Network,
        pi_levels_config: Option<&'a Vec<u32>>,
        count_buffer: Option<bool>,
    ) -> Vec<u32> {
        if let Some(levels) = pi_levels_config {
            assert_eq!(
                levels.len(),
                ntk.nb_inputs(),
                "Number of PI levels config must match number of PIs"
            );
        }

        LevelViewBuilder {
            ntk,
            pi_levels: pi_levels_config,
            count_buffer: count_buffer.unwrap_or(true),
            visited: FxHashSet::default(),
        }
        .build()
    }

    pub fn build(&mut self) -> Vec<u32> {
        let mut levels = vec![0; self.ntk.nb_nodes()];

        for po in 0..self.ntk.nb_outputs() {
            self.update(self.ntk.output(po).var(), &mut levels);
        }

        levels
    }

    fn fanin_level(&self, pi: u32) -> u32 {
        self.pi_levels
            .map(|levels| levels[pi as usize])
            .unwrap_or(0)
    }

    pub fn update(&mut self, node: u32, levels: &mut Vec<u32>) -> u32 {
        if self.visited.contains(&node) {
            return levels[node as usize];
        }
        self.visited.insert(node);

        let mut lv = 0;
        let g = self.ntk.gate(node as usize);
        for fanin in g.dependencies() {
            if fanin.is_input() {
                lv = lv.max(self.fanin_level(fanin.input()));
            } else if !fanin.is_constant() {
                lv = lv.max(self.update(fanin.var(), levels));
            }
        }

        let inc = if self.count_buffer {
            1
        } else {
            if let Gate::Buf(_) = g {
                0
            } else {
                1
            }
        };

        levels[node as usize] = lv + inc;
        levels[node as usize]
    }
}

struct ReverseLevelViewBuilder<'a> {
    ntk: &'a Network,
    fanout_view: &'a FanoutView,
    po_levels: Option<&'a Vec<u32>>,
    count_buffer: bool,
    visited: FxHashSet<u32>,
}

impl<'a> ReverseLevelViewBuilder<'a> {
    pub fn compute_levels(
        ntk: &'a Network,
        fanout_view: &'a FanoutView,
        po_levels_config: Option<&'a Vec<u32>>,
        count_buffer: Option<bool>,
    ) -> Vec<u32> {
        if let Some(levels) = po_levels_config {
            assert_eq!(
                levels.len(),
                ntk.nb_outputs(),
                "Number of PO levels config must match number of POs"
            );
        }

        ReverseLevelViewBuilder {
            ntk,
            fanout_view,
            po_levels: po_levels_config,
            count_buffer: count_buffer.unwrap_or(true),
            visited: FxHashSet::default(),
        }
        .build()
    }

    pub fn build(&mut self) -> Vec<u32> {
        let mut levels = vec![0; self.ntk.nb_nodes()];

        for pi in 0..self.ntk.nb_inputs() {
            self.update(self.ntk.input(pi), &mut levels);
        }

        levels
    }

    fn fanout_level(&self, po: u32) -> u32 {
        self.po_levels
            .map(|levels| levels[po as usize])
            .unwrap_or(0)
    }

    fn add_fanout_levels(&self, sig: Signal) -> u32 {
        if self.po_levels.is_none() || !sig.is_var()
        // || !self.fanout_view.fanouts(self.ntk.node(node as usize)).is_empty()
        {
            0
        } else {
            let idx = (0..self.ntk.nb_outputs()).find(|&po| self.ntk.output(po).var() == sig.var());
            idx.map(|idx| self.fanout_level(idx as u32)).unwrap_or(0)
        }
    }

    pub fn update(&mut self, sig: Signal, levels: &mut Vec<u32>) -> u32 {
        let mut lv = self.add_fanout_levels(sig);
        for &fanout in self.fanout_view.fanouts(sig) {
            lv = lv.max(self.update_node(fanout, levels));
        }
        lv
    }

    pub fn update_node(&mut self, node: u32, levels: &mut Vec<u32>) -> u32 {
        if self.visited.contains(&node) {
            return levels[node as usize];
        }
        self.visited.insert(node);

        let sig = self.ntk.node(node as usize);
        let lv = self.update(sig, levels);

        let mut inc = if self.count_buffer {
            1
        } else {
            if let Gate::Buf(_) = self.ntk.gate(node as usize) {
                0
            } else {
                1
            }
        };

        levels[node as usize] = lv + inc;
        levels[node as usize]
    }
}

/// Compute the levels of the network
///
/// # Arguments
/// * `ntk` - The network to compute the levels of
/// * `count_buffer` - Whether to count buffers as levels
///
/// # Returns
/// A vector of levels for each node in the network
pub fn compute_levels(ntk: &Network, count_buffer: bool) -> Vec<u32> {
    LevelViewBuilder::compute_levels(ntk, None, Some(count_buffer))
}

/// Compute the reverse levels of the network
///
/// # Arguments
/// * `ntk` - The network to compute the reverse levels of
/// * `count_buffer` - Whether to count buffers as levels
///
/// # Returns
/// A vector of reverse levels for each node in the network
pub fn compute_reverse_levels(ntk: &Network, count_buffer: bool) -> Vec<u32> {
    ReverseLevelViewBuilder::compute_levels(ntk, &FanoutView::new(ntk), None, Some(count_buffer))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_view() {
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
        // crate::io::write_dot_file(&std::path::PathBuf::from("level_view.dot"), &aig);

        let level_view = LevelViewBuilder::compute_levels(&aig, None, None);
        assert_eq!(level_view[0], 1);
        assert_eq!(level_view[1], 1);
        assert_eq!(level_view[2], 1);
        assert_eq!(level_view[3], 2);
        assert_eq!(level_view[4], 3);

        let level_view = LevelViewBuilder::compute_levels(&aig, Some(&vec![3, 2, 1, 0]), None);
        assert_eq!(level_view[0], 4);
        assert_eq!(level_view[1], 2);
        assert_eq!(level_view[2], 4);
        assert_eq!(level_view[3], 5);
        assert_eq!(level_view[4], 6);
    }

    #[test]
    fn test_level_view_with_buffer() {
        let mut aig = Network::default();
        let x1 = aig.add_input();
        let x2 = aig.add_input();
        let x3 = aig.add_input();
        let x4 = aig.add_input();

        let f1 = aig.and(x1, x2);
        let f2 = aig.and(x3, x4);
        let f3 = aig.and(x1, x3);
        let f4 = aig.and(f2, f3);
        let f5 = aig.and(f1, f2);
        let fb = aig.add(Gate::Buf(f4));
        let f6 = aig.and(f5, fb);
        let f7 = aig.add(Gate::Buf(f6));

        aig.add_output(f7);
        // crate::io::write_dot_file(&std::path::PathBuf::from("level_view.dot"), &aig);

        let level_view = LevelViewBuilder::compute_levels(&aig, None, None);
        assert_eq!(level_view[0], 1);
        assert_eq!(level_view[1], 1);
        assert_eq!(level_view[2], 1);
        assert_eq!(level_view[3], 2);
        assert_eq!(level_view[4], 2);
        assert_eq!(level_view[5], 3);
        assert_eq!(level_view[6], 4);
        assert_eq!(level_view[7], 5);

        let level_view = LevelViewBuilder::compute_levels(&aig, None, Some(false));
        assert_eq!(level_view[0], 1);
        assert_eq!(level_view[1], 1);
        assert_eq!(level_view[2], 1);
        assert_eq!(level_view[3], 2);
        assert_eq!(level_view[4], 2);
        assert_eq!(level_view[5], 2);
        assert_eq!(level_view[6], 3);
        assert_eq!(level_view[7], 3);
    }

    #[test]
    fn test_reverse_level_view() {
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
        // crate::io::write_dot_file(&std::path::PathBuf::from("level_view.dot"), &aig);

        let level_view =
            ReverseLevelViewBuilder::compute_levels(&aig, &FanoutView::new(&aig), None, None);
        assert_eq!(level_view[0], 3);
        assert_eq!(level_view[1], 3);
        assert_eq!(level_view[2], 2);
        assert_eq!(level_view[3], 2);
        assert_eq!(level_view[4], 1);

        let level_view = ReverseLevelViewBuilder::compute_levels(
            &aig,
            &FanoutView::new(&aig),
            Some(&vec![5, 1]),
            None,
        );

        assert_eq!(level_view[0], 4);
        assert_eq!(level_view[1], 4);
        assert_eq!(level_view[2], 6);
        assert_eq!(level_view[3], 3);
        assert_eq!(level_view[4], 2);
    }

    #[test]
    fn test_reverse_level_view_with_buffer() {
        let mut aig = Network::default();
        let x1 = aig.add_input();
        let x2 = aig.add_input();
        let x3 = aig.add_input();
        let x4 = aig.add_input();

        let f1 = aig.and(x1, x2);
        let f2 = aig.and(x3, x4);
        let f3 = aig.and(x1, x3);
        let f4 = aig.and(f2, f3);
        let f5 = aig.and(f1, f2);
        let fb = aig.add(Gate::Buf(f4));
        let f6 = aig.and(f5, fb);
        let f7 = aig.add(Gate::Buf(f6));

        aig.add_output(f7);
        // crate::io::write_dot_file(&std::path::PathBuf::from("level_view.dot"), &aig);

        let level_view =
            ReverseLevelViewBuilder::compute_levels(&aig, &FanoutView::new(&aig), None, None);
        assert_eq!(level_view[0], 4);
        assert_eq!(level_view[1], 5);
        assert_eq!(level_view[2], 5);
        assert_eq!(level_view[3], 4);
        assert_eq!(level_view[4], 3);
        assert_eq!(level_view[5], 3);
        assert_eq!(level_view[6], 2);
        assert_eq!(level_view[7], 1);

        let level_view = ReverseLevelViewBuilder::compute_levels(
            &aig,
            &FanoutView::new(&aig),
            None,
            Some(false),
        );
        assert_eq!(level_view[0], 3);
        assert_eq!(level_view[1], 3);
        assert_eq!(level_view[2], 3);
        assert_eq!(level_view[3], 2);
        assert_eq!(level_view[4], 2);
        assert_eq!(level_view[5], 1);
        assert_eq!(level_view[6], 1);
        assert_eq!(level_view[7], 0);
    }
}
