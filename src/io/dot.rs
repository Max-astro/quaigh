//! Write logic networks to DOT graph format

use std::io::Write;

use crate::network::{BinaryType, NaryType, TernaryType};
use crate::{Gate, Network, Signal};

/// Get a string representation of a gate type for DOT labels
fn gate_type_label(gate: &Gate) -> String {
    match gate {
        Gate::Binary(_, BinaryType::And) => "And2".to_string(),
        Gate::Binary(_, BinaryType::Xor) => "Xor2".to_string(),
        Gate::Ternary(_, TernaryType::And) => "And3".to_string(),
        Gate::Ternary(_, TernaryType::Xor) => "Xor3".to_string(),
        Gate::Ternary(_, TernaryType::Mux) => "Mux".to_string(),
        Gate::Ternary(_, TernaryType::Maj) => "Maj".to_string(),
        Gate::Nary(v, tp) => {
            let name = match tp {
                NaryType::And => "And",
                NaryType::Or => "Or",
                NaryType::Nand => "Nand",
                NaryType::Nor => "Nor",
                NaryType::Xor => "Xor",
                NaryType::Xnor => "Xnor",
            };
            format!("{}{}", name, v.len())
        }
        Gate::Buf(_) => "Buf".to_string(),
        Gate::Dff(_) => "Dff".to_string(),
        Gate::Lut(lut) => {
            // Convert truthtable to hex
            let num_bits = lut.lut.num_bits();
            let mut value: u64 = 0;
            for i in 0..num_bits {
                if lut.lut.value(i) {
                    value |= 1 << i;
                }
            }
            format!("Lut{}\\n0x{:X}", lut.lut.num_vars(), value)
        }
    }
}

/// Get the DOT node ID for a signal source
fn signal_source_id(s: &Signal) -> String {
    if s.is_constant() {
        if s.is_inverted() {
            "const_1".to_string()
        } else {
            "const_0".to_string()
        }
    } else if s.is_input() {
        format!("input_{}", s.input())
    } else {
        format!("node_{}", s.var())
    }
}

/// Write a network in DOT graph format
///
/// - Complementary edges are drawn with dashed lines
/// - Each node shows its Gate type (LUT gates also show truthtable in hex)
/// - Primary inputs use up triangle shape (▲)
/// - Primary outputs use down triangle shape (▼)
pub fn write_dot<W: Write>(w: &mut W, aig: &Network) {
    writeln!(w, "digraph network {{").unwrap();
    writeln!(w, "    rankdir=TB;").unwrap();
    writeln!(w, "    node [fontname=\"Helvetica\"];").unwrap();
    writeln!(w, "    edge [fontname=\"Helvetica\"];").unwrap();
    writeln!(w).unwrap();

    // Write constant nodes if they are used
    let mut has_const_0 = false;
    let mut has_const_1 = false;
    for i in 0..aig.nb_nodes() {
        for s in aig.gate(i).dependencies() {
            if s.is_constant() {
                if s.is_inverted() {
                    has_const_1 = true;
                } else {
                    has_const_0 = true;
                }
            }
        }
    }
    for i in 0..aig.nb_outputs() {
        let s = aig.output(i);
        if s.is_constant() {
            if s.is_inverted() {
                has_const_1 = true;
            } else {
                has_const_0 = true;
            }
        }
    }

    if has_const_0 || has_const_1 {
        writeln!(w, "    // Constant nodes").unwrap();
        if has_const_0 {
            writeln!(w, "    const_0 [label=\"0\" shape=plaintext fontsize=14];").unwrap();
        }
        if has_const_1 {
            writeln!(w, "    const_1 [label=\"1\" shape=plaintext fontsize=14];").unwrap();
        }
        writeln!(w).unwrap();
    }

    // Write primary inputs (down triangle)
    writeln!(w, "    // Primary inputs").unwrap();
    writeln!(w, "    subgraph cluster_inputs {{").unwrap();
    writeln!(w, "        rank=source;").unwrap();
    writeln!(w, "        style=invis;").unwrap();
    for i in 0..aig.nb_inputs() {
        writeln!(
            w,
            "        input_{} [label=\"i{}\" shape=invtriangle style=filled fillcolor=\"#90EE90\"];",
            i, i
        )
        .unwrap();
    }
    writeln!(w, "    }}").unwrap();
    writeln!(w).unwrap();

    // Write internal nodes
    writeln!(w, "    // Internal nodes").unwrap();
    for i in 0..aig.nb_nodes() {
        let gate = aig.gate(i);
        let label = gate_type_label(gate);
        let shape = if matches!(gate, Gate::Dff(_)) {
            "box"
        } else {
            "ellipse"
        };
        writeln!(
            w,
            "    node_{} [label=\"n{}\\n{}\" shape={}];",
            i, i, label, shape
        )
        .unwrap();
    }
    writeln!(w).unwrap();

    // Write primary outputs (up triangle)
    writeln!(w, "    // Primary outputs").unwrap();
    writeln!(w, "    subgraph cluster_outputs {{").unwrap();
    writeln!(w, "        rank=sink;").unwrap();
    writeln!(w, "        style=invis;").unwrap();
    for i in 0..aig.nb_outputs() {
        writeln!(
            w,
            "        output_{} [label=\"o{}\" shape=triangle style=filled fillcolor=\"#FFB6C1\"];",
            i, i
        )
        .unwrap();
    }
    writeln!(w, "    }}").unwrap();
    writeln!(w).unwrap();

    // Write edges from inputs to gates
    writeln!(w, "    // Edges").unwrap();
    for i in 0..aig.nb_nodes() {
        let gate = aig.gate(i);
        for (j, s) in gate.dependencies().iter().enumerate() {
            let src = signal_source_id(&s.without_inversion());
            let style = if s.is_inverted() {
                " [style=dashed]"
            } else {
                ""
            };
            // Add port label for gates with multiple inputs
            let edge_label = if gate.dependencies().len() > 1 {
                format!(" [{}]", if s.is_inverted() { " style=dashed" } else { "" })
            } else {
                style.to_string()
            };
            writeln!(w, "    {} -> node_{}{};", src, i, edge_label).unwrap();
        }
    }
    writeln!(w).unwrap();

    // Write edges to outputs
    writeln!(w, "    // Output edges").unwrap();
    for i in 0..aig.nb_outputs() {
        let s = aig.output(i);
        let src = signal_source_id(&s.without_inversion());
        let style = if s.is_inverted() {
            " [style=dashed]"
        } else {
            ""
        };
        writeln!(w, "    {} -> output_{}{};", src, i, style).unwrap();
    }

    writeln!(w, "}}").unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufWriter;

    #[test]
    fn test_write_dot_basic() {
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
        aig.add_output(!f5);

        let mut buf = BufWriter::new(Vec::new());
        write_dot(&mut buf, &aig);
        let dot = String::from_utf8(buf.into_inner().unwrap()).unwrap();

        // println!("{}", dot);
        // Check that it contains expected elements
        assert!(dot.contains("digraph network"));
        assert!(dot.contains("shape=triangle")); // inputs
        assert!(dot.contains("shape=invtriangle")); // outputs
        assert!(dot.contains("And2")); // gate type
        assert!(dot.contains("style=dashed")); // inverted edges
    }
}
