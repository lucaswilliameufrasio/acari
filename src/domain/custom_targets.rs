use crate::domain::CleanTarget;

pub fn append_custom_scan_paths(targets: &mut Vec<CleanTarget>, scan_paths: &[String]) {
    for (idx, path) in scan_paths.iter().enumerate() {
        let name: &'static str = Box::leak(format!("Custom Path {}", idx + 1).into_boxed_str());
        let leaked_path: &'static str = Box::leak(path.clone().into_boxed_str());

        targets.push(CleanTarget {
            name,
            path: leaked_path,
            description: "User provided path",
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::CleanTarget;

    use super::append_custom_scan_paths;

    #[test]
    fn appends_custom_paths_with_stable_names() {
        let mut targets: Vec<CleanTarget> = Vec::new();
        let scan_paths = vec![String::from("/tmp/one"), String::from("/tmp/two")];

        append_custom_scan_paths(&mut targets, &scan_paths);

        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].name, "Custom Path 1");
        assert_eq!(targets[0].path, "/tmp/one");
        assert_eq!(targets[1].name, "Custom Path 2");
        assert_eq!(targets[1].path, "/tmp/two");
        assert_eq!(targets[0].description, "User provided path");
    }
}
