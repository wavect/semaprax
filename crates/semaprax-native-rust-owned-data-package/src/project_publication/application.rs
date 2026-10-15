//! Bounded application inventories use the same held stage and publication owner.
use super::*;

pub(super) trait ApplicationFiles {
    fn write(
        &mut self,
        directory: &platform::HeldDirectory,
        name: &str,
        bytes: &[u8],
    ) -> Result<(), platform::Error>;
    fn authenticate(
        &self,
        directory: &platform::HeldDirectory,
        files: &[(&str, &[u8])],
        published: bool,
    ) -> Result<(), NewProjectAuthorityError>;
    fn settle(&mut self) -> Result<(), platform::Error>;
    fn discard(
        &self,
        stage: &platform::HeldDirectory,
        source: &platform::HeldDirectory,
        name: &platform::PreparedStageName,
    ) -> Result<(), platform::Error>;
}

struct Files<const N: usize> {
    names: [String; N],
    inventory: platform::PreparedDiscardInventory<N>,
}

impl<const N: usize> ApplicationFiles for Files<N> {
    fn write(
        &mut self,
        directory: &platform::HeldDirectory,
        name: &str,
        bytes: &[u8],
    ) -> Result<(), platform::Error> {
        platform::write_file_new_prepared(directory, &mut self.inventory, name, bytes, 0o600)
    }
    fn authenticate(
        &self,
        directory: &platform::HeldDirectory,
        files: &[(&str, &[u8])],
        published: bool,
    ) -> Result<(), NewProjectAuthorityError> {
        if files.len() != N
            || files
                .iter()
                .zip(&self.names)
                .any(|((path, _), name)| path.strip_prefix("src/") != Some(name.as_str()))
        {
            return Err(NewProjectAuthorityError::Invalid);
        }
        if published {
            let held = files
                .iter()
                .zip(&self.names)
                .map(|((_, bytes), name)| hold_matching(directory, OsStr::new(name), bytes))
                .collect::<Result<Vec<_>, _>>()?;
            let mut scan = platform::prepare_inventory_entries_exact(
                self.names.each_ref().map(|name| OsStr::new(name)),
                N,
            )
            .map_err(map_invalid)?;
            let references: [&platform::HeldRegularFile; N] = std::array::from_fn(|i| &held[i]);
            platform::inventory_entries_exact_prepared(&mut scan, directory, references, [])
                .map_err(map_changed)
        } else {
            for ((_, bytes), name) in files.iter().zip(&self.names) {
                authenticate_file(self.inventory.file(name).map_err(map_changed)?, bytes)?;
            }
            let mut scan =
                platform::prepare_inventory_exact(&self.inventory).map_err(map_invalid)?;
            platform::inventory_exact_prepared(&mut scan, directory, &self.inventory)
                .map_err(map_changed)
        }
    }
    fn settle(&mut self) -> Result<(), platform::Error> {
        self.inventory.settle_for_publish()
    }
    fn discard(
        &self,
        stage: &platform::HeldDirectory,
        source: &platform::HeldDirectory,
        name: &platform::PreparedStageName,
    ) -> Result<(), platform::Error> {
        platform::discard_owned_stage_prepared(stage, source, name, &self.inventory)
    }
}

fn prepare<const N: usize>(
    paths: &[String],
) -> Result<Box<dyn ApplicationFiles>, NewProjectAuthorityError> {
    let names: [String; N] = std::array::from_fn(|i| {
        paths[i]
            .strip_prefix("src/")
            .expect("validated source path")
            .to_owned()
    });
    let inventory =
        platform::prepare_owned_discard_inventory(names.each_ref().map(|name| OsStr::new(name)))
            .map_err(map_invalid)?;
    Ok(Box::new(Files { names, inventory }))
}

pub(super) fn inventory(
    paths: &[String],
) -> Result<Box<dyn ApplicationFiles>, NewProjectAuthorityError> {
    if paths.iter().any(|path| {
        path.strip_prefix("src/").is_none_or(|name| {
            name.is_empty() || name.contains('/') || name.contains('\\') || !name.ends_with(".spx")
        })
    }) || paths.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(NewProjectAuthorityError::Invalid);
    }
    macro_rules! arities { ($($n:literal),*) => { match paths.len() { $($n => prepare::<$n>(paths),)* _ => Err(NewProjectAuthorityError::Invalid) } }; }
    arities!(
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
        26, 27, 28, 29, 30, 31, 32
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn application_inventory_is_closed_sorted_unique_and_bounded() {
        for count in [1, 32] {
            let paths = (0..count)
                .map(|i| format!("src/module{i:02}.spx"))
                .collect::<Vec<_>>();
            assert!(inventory(&paths).is_ok());
        }
        for paths in [
            vec![],
            vec!["src/a.spx".to_owned(); 33],
            vec!["src/a.spx".to_owned(); 2],
            vec!["src/b.spx".to_owned(), "src/a.spx".to_owned()],
            vec!["../outside.spx".to_owned()],
            vec!["src/../outside.spx".to_owned()],
            vec!["src/a/b.spx".to_owned()],
        ] {
            assert!(inventory(&paths).is_err());
        }
    }
}
