//! Optimization by resubstituting nodes with existing signals

use crate::{Gate, Network, Signal};

/// Subsitute a node with an exist signal
pub fn substitute_node(ntk: &mut Network, node: usize, new_signal: Signal) {
    ntk.replace(node, Gate::Buf(new_signal));
}

#[cfg(test)]
mod tests {
    use super::substitute_node;
    use crate::Network;

    #[test]
    fn test_substitute_node_1() {
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
        assert_eq!(aig.nb_nodes(), 5);

        substitute_node(&mut aig, f2.var() as usize, x3);
        aig.make_canonical();
        assert_eq!(aig.nb_nodes(), 4);
    }

    #[test]
    fn test_substitute_node_2() {
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
        assert_eq!(aig.nb_nodes(), 5);

        substitute_node(&mut aig, f5.var() as usize, f3);
        aig.make_canonical();
        aig.cleanup();
        assert_eq!(aig.nb_nodes(), 1);
    }
}
