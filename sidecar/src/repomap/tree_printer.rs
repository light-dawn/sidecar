use crate::chunking::languages::TSLanguageParsing;
use std::collections::{HashMap, HashSet};
use thiserror::Error;
use tree_sitter::{Node, Tree, TreeCursor};

#[derive(Debug, Error)]
pub enum TreePrinterError {
    #[error("No language configuration found for file: {0}")]
    MissingConfig(String),
    #[error("Failed to parse tree for file: {0}")]
    ParseError(String),
    #[error("Invalid tree structure: {0}")]
    InvalidTree(String),
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

pub struct TreePrinter<'a> {
    tree: &'a Tree,
    code: String,
    line_number: bool,
    parent_context: bool,
    child_context: bool,
    last_line: bool,
    margin: usize,
    mark_lois: bool,
    header_max: usize,
    show_top_of_file_parent_scope: bool,
    loi_pad: usize,
    scopes: Vec<HashSet<usize>>, // the starting lines of the nodes that span the line
    header: Vec<Vec<(usize, usize, usize)>>, // the size, start line, end line of the nodes that span the line
    nodes: Vec<Vec<Node<'a>>>,               // tree-sitter node requires lifetime parameter
    num_lines: usize,
    output: Vec<String>,
}

impl<'a> TreePrinter<'a> {
    pub fn new(tree: &'a Tree, code: String) -> Result<Self, TreePrinterError> {
        let num_lines = code.lines().count();

        Ok(Self {
            tree,
            code,
            line_number: false,
            parent_context: true,
            child_context: true,
            last_line: true,
            margin: 3,
            mark_lois: true,
            header_max: 10,
            show_top_of_file_parent_scope: false,
            loi_pad: 1,
            scopes: vec![HashSet::new(); num_lines],
            header: vec![Vec::new(); num_lines],
            nodes: vec![Vec::new(); num_lines],
            num_lines,
            output: vec![],
        })
    }

    fn print_node_at_cursor(cursor: &TreeCursor) {
        println!("Node type: {}", cursor.node().kind());
        println!("Node field_name: {:?}", cursor.field_name());
    }

    /// TreeCursor shares lifetime with self.nodes
    pub fn walk_tree(&mut self, cursor: &mut TreeCursor<'a>) {
        let node = cursor.node();

        let start_line = node.start_position().row;
        let end_line = node.end_position().row;
        let size = end_line - start_line;

        self.nodes[start_line].push(node);

        if size > 0 {
            self.header[start_line].push((size, start_line, end_line));
        }

        // for every line the node spans, add the position of its start line
        for i in start_line..=end_line {
            self.scopes[i].insert(start_line);
        }

        if cursor.goto_first_child() {
            loop {
                self.walk_tree(cursor);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
        }
    }

    // walk tree

    // add lines of interest (lois)

    // add context()

    // format
}
