//! Strongly connected components: the sets of nodes in which each reaches
//! every other (Tarjan's algorithm, with a stack of its own instead of
//! recursion, so a long chain cannot overflow the thread's).

/// Each component of two or more nodes, sorted, in order of their first node.
/// `edges[node]` are the nodes `node` reaches in one step.
pub fn circles(edges: &[Vec<usize>]) -> Vec<Vec<usize>> {
    const UNSEEN: usize = usize::MAX;
    let count = edges.len();
    let mut index = vec![UNSEEN; count];
    let mut low = vec![0; count];
    let mut on_stack = vec![false; count];
    let mut stack = Vec::new();
    let mut next = 0;
    let mut found = Vec::new();
    for start in 0..count {
        if index[start] != UNSEEN {
            continue;
        }
        let mut work = vec![(start, 0)];
        index[start] = next;
        low[start] = next;
        next += 1;
        stack.push(start);
        on_stack[start] = true;
        while let Some(&(node, edge)) = work.last() {
            if let Some(&to) = edges[node].get(edge) {
                work.last_mut().expect("the node just seen").1 += 1;
                if index[to] == UNSEEN {
                    index[to] = next;
                    low[to] = next;
                    next += 1;
                    stack.push(to);
                    on_stack[to] = true;
                    work.push((to, 0));
                } else if on_stack[to] {
                    low[node] = low[node].min(index[to]);
                }
                continue;
            }
            work.pop();
            if let Some(&(parent, _)) = work.last() {
                low[parent] = low[parent].min(low[node]);
            }
            if low[node] == index[node] {
                let mut component = Vec::new();
                while let Some(member) = stack.pop() {
                    on_stack[member] = false;
                    component.push(member);
                    if member == node {
                        break;
                    }
                }
                if component.len() > 1 {
                    component.sort_unstable();
                    found.push(component);
                }
            }
        }
    }
    found.sort();
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_each_circle_and_leaves_out_the_rest() {
        // 0 -> 1 -> 2 -> 0, 2 -> 3, 3 -> 4 -> 3, 5 alone.
        let edges = [vec![1], vec![2], vec![0, 3], vec![4], vec![3], vec![]];
        assert_eq!(circles(&edges), [vec![0, 1, 2], vec![3, 4]]);
    }
}
