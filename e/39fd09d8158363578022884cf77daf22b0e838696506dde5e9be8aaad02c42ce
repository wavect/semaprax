use super::{before_alias, DeclIter, DeclSlices, DeclarationDirectory};
struct Directory(Vec<i32>);
impl DeclarationDirectory<i32> for Directory {
    fn len(&self) -> usize {
        self.0.len()
    }
    fn get(&self, index: usize) -> Option<&i32> {
        Some(&self.0[index])
    }
}
#[test]
#[allow(
    clippy::useless_vec,
    reason = "The control deliberately owns Vec backing while observing borrowed declaration slices."
)]
fn ordinary_and_projected_views_keep_complete_old_size_order_suffix_and_iteration() {
    assert!(
        std::mem::size_of::<DeclSlices<'_, i32>>()
            <= std::mem::size_of::<before_alias::DeclSlices<'_, i32>>()
    );
    assert!(
        std::mem::size_of::<DeclIter<'_, i32>>()
            <= std::mem::size_of::<before_alias::DeclIter<'_, i32>>()
    );
    for prefix in 0..5 {
        for suffix in 0..5 {
            let authored = vec![1, 2, 3, 4];
            let generated = vec![5, 6, 7, 8];
            let directory = Directory(generated[..suffix].to_vec());
            let old = before_alias::DeclSlices::append(&authored[..prefix], &generated[..suffix])
                .unwrap();
            let ordinary = DeclSlices::append(&authored[..prefix], &generated[..suffix]).unwrap();
            let projected = DeclSlices::projected(&authored[..prefix], &directory).unwrap();
            for start in 0..=prefix + suffix + 1 {
                for current in [&ordinary, &projected] {
                    assert_eq!(current.len(), old.len());
                    assert_eq!(
                        current.iter().copied().collect::<Vec<_>>(),
                        old.iter().copied().collect::<Vec<_>>()
                    );
                    assert_eq!(
                        current.iter().rev().copied().collect::<Vec<_>>(),
                        old.iter().rev().copied().collect::<Vec<_>>()
                    );
                    assert_eq!(current.get(start), old.get(start));
                    assert_eq!(
                        current
                            .suffix(start)
                            .map(|v| v.iter().copied().collect::<Vec<_>>()),
                        old.suffix(start)
                            .map(|v| v.iter().copied().collect::<Vec<_>>())
                    );
                }
            }
        }
    }
    let values = [10, 20, 30, 40];
    let selected = DeclSlices::selected(&values, &[0, 2], &[50]).unwrap();
    assert_eq!(selected.iter().copied().collect::<Vec<_>>(), [10, 30, 50]);
    assert!(DeclSlices::selected(&values, &[2, 0], &[]).is_none());
    assert!(DeclSlices::selected(&values, &[0, 4], &[]).is_none());
}
