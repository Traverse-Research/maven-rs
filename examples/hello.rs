use maven_toolbox::{default_impl::*, *};
use parking_lot::Mutex;
use std::sync::Arc;

struct MultiRepositoryResolver {
    resolvers: Vec<Arc<Resolver>>,
}

impl MultiRepositoryResolver {
    pub fn build_effective_pom<UF, P>(
        &mut self,
        project_id: &ArtifactFqn,
        url_fetcher: &UF,
        pom_parser: &P,
    ) -> Result<Project, ResolverError>
    where
        UF: UrlFetcher,
        P: PomParser,
    {
        for resolver in &mut self.resolvers {
            match resolver
                .clone()
                .build_effective_pom(project_id, url_fetcher, pom_parser)
            {
                Ok(project) => return Ok(project),
                Err(ResolverError {
                    kind: ErrorKind::FileNotFound,
                    ..
                }) => continue,
                err => return err,
            }
        }

        Err(ResolverError::file_not_found(&format!("Project: {}", project_id)))
    }
}

fn main() {
    let artifact = ArtifactFqn::pom(
        "androidx.appcompat",
        "appcompat",
        "1.7.0", 
        // "org.jetbrains.kotlin",
        // "kotlin-stdlib",
        // "1.8.22",
    );

    println!("Resolving {}...", artifact);

    // let mut resolver = Resolver::default();
    let mut resolver = Resolver {
        repositories: vec![
            Arc::new(Repository {
                base_url: "https://dl.google.com/dl/android/maven2".to_string(),
            }),
            Arc::new(Repository {
                base_url: "https://repo.maven.apache.org/maven2".into(),
            }),
        ],
        project_cache: Mutex::new(std::collections::HashMap::new()),
    };

    let url_fetcher = DefaultUrlFetcher {};
    let pom_parser = DefaultPomParser {};

    
    // print out all dependencies with "compile" scope
    std::fs::create_dir_all("classes");
    
    let mut todo = std::collections::VecDeque::new();
    todo.push_back(artifact);

    let mut done = std::collections::HashSet::new();
    
    while let Some(artifact) = todo.pop_front() {
        if !done.insert(artifact.clone()) {
            continue
        }

        let project = resolver
            .build_effective_pom(&artifact, &url_fetcher, &pom_parser)
            .unwrap();

        let package = resolver
            .try_download_package(&project.artifact_fqn, &url_fetcher)
            .unwrap();

        package.extract_jar_file(std::path::Path::new(&format!(
            "classes/{}-{}.jar",
            project.artifact_fqn.artifact_id.as_ref().unwrap(),
            project.artifact_fqn.version.as_ref().unwrap()
        )));


        for dep in project
            .dependencies
            .values()
            .filter(|dep| dep.scope.as_deref() == Some("compile")) 
        {
            todo.push_back(dep.artifact_fqn.clone());
        }
    }

    for artifact_fqn in done {

        let path = std::path::PathBuf::from(format!(
            "classes/{}-{}.jar",
            artifact_fqn.artifact_id.as_ref().unwrap(),
            artifact_fqn.version.as_ref().unwrap()
        ));

        println!("{:?}", &path.canonicalize());
    }
}
