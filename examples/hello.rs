use maven::*;
use std::path::{PathBuf, Path};

fn main() {
    let start = std::time::Instant::now();
    
    let resolver = Resolver::new(
        &[
            Repository::google_maven(),
            Repository::maven_central(),
        ],
    );

    let done = resolver.download_all_jars(
        &[
            Artifact::pom("androidx.appcompat", "appcompat", "1.7.0"),
            Artifact::pom("androidx.games", "games-activity", "2.0.2"),
        ],
        Path::new("classes"),
    );

    println!("{:?}", start.elapsed());

    let mut class_path: Vec<String> = vec![];

    for artifact_fqn in done {
        let path = PathBuf::from("classes/").join(artifact_fqn.filename());

        class_path.push(path.canonicalize().unwrap().display().to_string());
    }

    println!("{}", class_path.join(":"));
}
