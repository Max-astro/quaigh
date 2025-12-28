use crate::{Network, Signal};

/// View of the fanout of each node in a network
/// Since quaigh network cannot access a node's fanout nodes directly,
/// this view provides a way to get the fanout of each node.
#[derive(Debug, Clone, Default)]
pub struct FanoutView {
    pi_fanout: Vec<Vec<u32>>,
    node_fanout: Vec<Vec<u32>>,
}

impl FanoutView {
    pub fn new(ntk: &Network) -> Self {
        let mut fanout_view = FanoutView {
            pi_fanout: vec![vec![]; ntk.nb_inputs()],
            node_fanout: vec![vec![]; ntk.nb_nodes()],
        };

        for i in 0..ntk.nb_nodes() {
            let g = ntk.gate(i);
            for fanin in g.dependencies() {
                if fanin.is_input() {
                    fanout_view.pi_fanout[fanin.input() as usize].push(i as u32);
                } else if !fanin.is_constant() {
                    fanout_view.node_fanout[fanin.var() as usize].push(i as u32);
                }
            }
        }
        fanout_view
    }

    pub fn fanouts(&self, s: Signal) -> &[u32] {
        if s.is_input() {
            &self.pi_fanout[s.input() as usize]
        } else if !s.is_constant() {
            &self.node_fanout[s.var() as usize]
        } else {
            &[]
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path;

    use super::*;

    fn verify_fanout_view(fanout_view: &FanoutView, ntk: &Network) {
        assert_eq!(fanout_view.pi_fanout.len(), ntk.nb_inputs());
        assert_eq!(fanout_view.node_fanout.len(), ntk.nb_nodes());

        assert!(
            fanout_view.pi_fanout.iter().all(|v| v.is_sorted()),
            "PI fanouts are not sorted"
        );
        assert!(
            fanout_view.node_fanout.iter().all(|v| v.is_sorted()),
            "Node fanouts are not sorted"
        );

        for pi in 0..ntk.nb_inputs() {
            for &node in fanout_view.fanouts(ntk.input(pi)) {
                assert!(
                    ntk.gate(node as usize)
                        .dependencies()
                        .iter()
                        .any(|s| s.is_input() && s.input() == pi as u32),
                    "Node {node} is not a fanout of Input {pi}"
                );
            }
        }
        for i in 0..ntk.nb_nodes() {
            for &fanout in fanout_view.fanouts(ntk.node(i)) {
                assert!(
                    ntk.gate(fanout as usize)
                        .dependencies()
                        .iter()
                        .any(|s| s.is_var() && s.var() == i as u32),
                    "Node {fanout} is not a fanout of node {i}"
                );
            }
        }
    }

    #[test]
    fn test_fanout_view() {
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

        aig.add_output(f5);

        // println!("{}", aig);

        let fanout_view = FanoutView::new(&aig);
        assert_eq!(fanout_view.pi_fanout[0], vec![0, 2]);
        assert_eq!(fanout_view.pi_fanout[1], vec![0]);
        assert_eq!(fanout_view.pi_fanout[2], vec![1, 2]);
        assert_eq!(fanout_view.pi_fanout[3], vec![1]);
        assert_eq!(fanout_view.node_fanout[0], vec![3]);
        assert_eq!(fanout_view.node_fanout[1], vec![3]);
        assert_eq!(fanout_view.node_fanout[2], vec![4]);
        assert_eq!(fanout_view.node_fanout[3], vec![4]);
        assert_eq!(fanout_view.node_fanout[4], vec![]);

        verify_fanout_view(&fanout_view, &aig);
    }

    // #[test]
    // fn test_fanout_view_large() {
    //     use crate::io::read_network_file;
    //     // let ntk = read_network_file(&path::PathBuf::from("benchmarks/epfl/ctrl/int2float.blif"));
    //     // let ntk = read_network_file(&path::PathBuf::from(
    //     //     "benchmarks/epfl/arithmetic/adder.blif",
    //     // ));
    //     let ntk = read_network_file(&path::PathBuf::from("benchmarks/blif/b01.blif"));

    //     // crate::io::write_dot_file(&path::PathBuf::from("int2float.dot"), &ntk);
    //     let fanout_view = FanoutView::new(&ntk);
    //     verify_fanout_view(&fanout_view, &ntk);
    // }
}
