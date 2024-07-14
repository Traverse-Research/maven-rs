use log::{debug, trace};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::cell::RefCell;

#[cfg(feature = "default-impl")]
pub mod default_impl;

pub enum Packaging {
    Aar(bytes::Bytes),
    Jar(bytes::Bytes),
}

impl Packaging {
    pub fn extract_jar_file(
        &self,
        location: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Self::Aar(bytes) => {
                use std::io::Read;

                let zip_file = zip::read::ZipArchive::new(std::io::Cursor::new(bytes));
                let mut bytes = vec![];
                zip_file?.by_name("classes.jar")?.read_to_end(&mut bytes)?;
                std::fs::write(location, bytes)?;
                Ok(())
            }
            Self::Jar(bytes) => {
                std::fs::write(location, bytes)?;
                Ok(())
            }
        }
    }
}

#[derive(Default, Debug, Clone, Eq, PartialEq, Hash)]
pub struct Artifact {
    pub group_id: Option<String>,
    pub artifact_id: Option<String>,
    pub version: Option<String>,
    pub packaging: Option<String>,
    pub classifier: Option<String>,
}

impl Artifact {
    pub fn new(
        group_id: &str,
        artifact_id: &str,
        version: &str,
        packaging: &str,
        classifier: &str,
    ) -> Self {
        Artifact {
            group_id: Some(group_id.to_owned()),
            artifact_id: Some(artifact_id.to_owned()),
            version: Some(version.to_owned()),
            packaging: Some(packaging.to_owned()),
            classifier: Some(classifier.to_owned()),
            ..Default::default()
        }
    }

    pub fn version_cleaned(&self) -> Option<String> {
        self.version
            .clone()
            .map(|version| version.replace("[", "").replace("]", ""))
    }

    pub fn pom(group_id: &str, artifact_id: &str, version: &str) -> Self {
        Artifact {
            group_id: Some(group_id.to_owned()),
            artifact_id: Some(artifact_id.to_owned()),
            version: Some(version.to_owned()),
            packaging: Some("pom".to_owned()),
            ..Default::default()
        }
    }

    pub fn interpolate(&self, properties: &HashMap<String, String>) -> Self {
        // TODO other fields
        Artifact {
            version: self
                .version
                .clone()
                .filter(|v| v.contains("${"))
                .map(|mut s| {
                    if let Some(start) = s.find("${") {
                        if let Some(end) = s[start..].find("}") {
                            let expr = s[start + 2..end].to_owned();
                            if let Some(v) = properties.get(&expr) {
                                s.replace_range(start..end + 1, v);
                            }
                        }
                    }
                    s
                })
                .or_else(|| self.version.clone()),
            ..self.clone()
        }
    }

    pub fn with_packaging(&self, packaging: &str) -> Self {
        Artifact {
            packaging: Some(packaging.to_owned()),
            ..self.clone()
        }
    }

    pub fn same_ga(&self, other: &Self) -> bool {
        self.group_id == other.group_id && self.artifact_id == other.artifact_id
    }

    pub fn normalize(self, parent: &Self, default_packaging: &str) -> Self {
        Artifact {
            group_id: self.group_id.or_else(|| parent.group_id.clone()),
            artifact_id: self.artifact_id.or_else(|| parent.artifact_id.clone()),
            version: self.version.or_else(|| parent.version.clone()),
            packaging: self
                .packaging
                .or_else(|| Some(default_packaging.to_owned())),
            ..Default::default()
        }
    }

    pub fn filename(&self) -> PathBuf {
        PathBuf::from(format!(
            "{}/{}.{}",
            self.artifact_id.as_ref().unwrap(),
            self.version_cleaned().as_ref().unwrap(),
            self.packaging.as_ref().unwrap()
        ))
    }
}

impl std::fmt::Display for Artifact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let def = "?".to_owned();
        write!(
            f,
            "{}:{}:{}:{}:{}",
            self.group_id.as_ref().unwrap_or(&def),
            self.artifact_id.as_ref().unwrap_or(&def),
            self.version.as_ref().unwrap_or(&def),
            self.packaging.as_ref().unwrap_or(&def),
            self.classifier.as_ref().unwrap_or(&def)
        )
    }
}

#[derive(Default, Debug, Clone)]
pub struct Dependency {
    pub artifact_fqn: Artifact,
    pub scope: Option<String>,
}

impl Dependency {
    pub fn get_key(&self) -> DependencyKey {
        DependencyKey {
            group_id: self.artifact_fqn.group_id.clone(),
            artifact_id: self.artifact_fqn.artifact_id.clone(),
        }
    }

    pub fn normalize(self, parent_id: &Artifact, default_packaging: &str) -> Self {
        Dependency {
            artifact_fqn: self.artifact_fqn.normalize(parent_id, default_packaging),
            scope: self.scope.or_else(|| Some("compile".to_owned())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Parent {
    pub artifact_fqn: Artifact,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DependencyKey {
    pub group_id: Option<String>,
    pub artifact_id: Option<String>,
}

impl std::fmt::Display for DependencyKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let def = "?".to_owned();
        write!(
            f,
            "{}:{}",
            self.group_id.as_ref().unwrap_or(&def),
            self.artifact_id.as_ref().unwrap_or(&def)
        )
    }
}

#[derive(Debug, Clone)]
pub struct DependencyManagement {
    pub dependencies: HashMap<DependencyKey, Dependency>,
}

#[derive(Debug, Clone)]
pub struct Project {
    pub parent: Option<Parent>,
    pub artifact_fqn: Artifact,
    pub dependency_management: Option<DependencyManagement>,
    pub dependencies: HashMap<DependencyKey, Dependency>,
    pub properties: HashMap<String, String>,
}

pub struct Repository {
    pub base_url: String,
}

impl Repository {
    pub fn google_maven() -> Arc<Self> {
        let base_url = "https://dl.google.com/dl/android/maven2";
        Arc::new(Self {
            base_url: base_url.to_string(),
        })
    }

    pub fn maven_central() -> Arc<Self> {
        Arc::new(Repository {
            base_url: "https://repo.maven.apache.org/maven2".into(),
        })
    }
}

#[derive(Debug)]
pub enum ErrorKind {
    ClientError,
    FileNotFound,
    // RepositoryError,
}

#[derive(Debug)]
pub struct ResolverError {
    pub kind: ErrorKind,
    pub msg: String,
}

impl ResolverError {
    pub fn missing_parameter<D: std::fmt::Display>(fqn: &Artifact, field_name: &D) -> Self {
        ResolverError {
            kind: ErrorKind::ClientError,
            msg: format!("'{}' is missing from {}", field_name, fqn),
        }
    }

    pub fn invalid_data(details: &str) -> Self {
        ResolverError {
            kind: ErrorKind::ClientError,
            msg: format!("Invalid input data: {}", details),
        }
    }

    pub fn cant_resolve(artifact_id: &Artifact, cause: &str) -> Self {
        ResolverError {
            kind: ErrorKind::ClientError,
            msg: format!("Can't resolve {:?}: {}", artifact_id, cause),
        }
    }

    pub fn file_not_found(url: &str) -> Self {
        ResolverError {
            kind: ErrorKind::FileNotFound,
            msg: format!("Can't find {}", url),
        }
    }
}

pub trait UrlFetcher {
    fn fetch(&self, url: &str) -> Result<String, ResolverError>;
    fn fetch_bytes(&self, url: &str) -> Result<bytes::Bytes, ResolverError>;
}

pub trait PomParser {
    fn parse(&self, input: String) -> Result<Project, ResolverError>;
}

pub struct Resolver {
    pub repositories: Vec<Arc<Repository>>,
    pub project_cache: RefCell<HashMap<Artifact, Project>>,

    url_fetcher: Box<dyn UrlFetcher>,
    pom_parser: Box<dyn PomParser>,
}

impl Default for Resolver {
    fn default() -> Self {
        Resolver {
            repositories: vec![Repository::maven_central()],
            project_cache: RefCell::new(HashMap::new()),
            url_fetcher: Box::new(default_impl::DefaultUrlFetcher {}),
            pom_parser: Box::new(default_impl::DefaultPomParser {})
        }
    }
}

fn normalize_gavs(
    dependencies: HashMap<DependencyKey, Dependency>,
    parent_fqn: &Artifact,
    default_packaging: &str,
) -> HashMap<DependencyKey, Dependency> {
    dependencies
        .into_iter()
        .map(|(_, dep)| {
            let dep = dep.normalize(parent_fqn, default_packaging);
            (dep.get_key(), dep)
        })
        .collect()
}

impl Resolver {
    pub fn new(repositories: &[Arc<Repository>]) -> Self {
        Self {
            repositories: repositories.to_vec(),
            project_cache: RefCell::new(HashMap::new()),
            url_fetcher: Box::new(default_impl::DefaultUrlFetcher {}),
            pom_parser: Box::new(default_impl::DefaultPomParser {})
        }
    }

    pub fn try_download_package(
        &self,
        id: &Artifact,
    ) -> Result<Packaging, ResolverError>
    {
        for repository in &self.repositories {
            for packaging in ["aar", "jar"] {
                let packaged_id = id.with_packaging(packaging);
                let url = Self::create_url_with_repository(repository, &packaged_id)?;
                match self.url_fetcher.fetch_bytes(&url) {
                    Ok(bytes) => {
                        return Ok(match packaging {
                            "aar" => Packaging::Aar(bytes),
                            "jar" => Packaging::Jar(bytes),
                            _ => unimplemented!("Unsupported packaging type {packaging}"),
                        });
                    }
                    err => debug!("Trying other packaging: {:?}", err),
                }
            }
        }

        Err(ResolverError::file_not_found(&format!(
            "{}",
            id.artifact_id.as_ref().unwrap()
        )))
    }

    pub fn create_url_with_repository(
        repository: &Repository,
        id: &Artifact,
    ) -> Result<String, ResolverError> {
        // a little helper
        fn require<'a, F, D>(
            id: &'a Artifact,
            f: F,
            field_name: &D,
        ) -> Result<&'a String, ResolverError>
        where
            F: Fn(&Artifact) -> Option<&String>,
            D: std::fmt::Display,
        {
            f(id).ok_or_else(|| ResolverError::missing_parameter(id, field_name))
        }

        let group_id = require(id, |id| id.group_id.as_ref(), &"groupId")?;
        let artifact_id = require(id, |id| id.artifact_id.as_ref(), &"artifactId")?;
        let _version = require(id, |id| id.version.as_ref(), &"version")?;
        let packaging = id.packaging.as_ref().map(|s| s.as_str()).unwrap_or("jar");

        let version = id.version_cleaned().unwrap();

        let mut url = format!(
            "{}/{}/{}/{}/{}-{}",
            repository.base_url,
            group_id.replace(".", "/"),
            artifact_id,
            version,
            artifact_id,
            version
        );

        if let Some(classifier) = &id.classifier {
            url += &format!("-{}", classifier);
        }

        url += &format!(".{}", packaging);

        Ok(url)
    }

    pub fn build_effective_pom(
        &self,
        project_id: &Artifact,
    ) -> Result<Project, ResolverError>
    {
        debug!("building an effective pom for {}", project_id);

        let project_id = &project_id.with_packaging("pom");
        for repository in &self.repositories {
            let Ok(mut project) =
                self.fetch_project(repository, project_id)
            else {
                continue;
            };

            if let Some(version) = &project_id.version {
                project
                    .properties
                    .insert("project.version".to_owned(), version.clone());
            }

            // merge in the dependencies from the parent POM
            if let Some(parent) = &project.parent {
                let parent_project =
                    self.build_effective_pom(&parent.artifact_fqn)?;

                trace!("got a parent POM: {}", parent_project.artifact_fqn);

                let extra_deps = parent_project
                    .dependencies
                    .into_iter()
                    .filter(|(dep_key, _)| !project.dependencies.contains_key(dep_key))
                    .collect::<HashMap<_, _>>();

                project.dependencies.extend(extra_deps);
            }

            if let Some(mut project_dm) = project.dependency_management.clone() {
                for (_, dep) in &mut project_dm.dependencies {
                    dep.artifact_fqn = dep.artifact_fqn.interpolate(&project.properties);
                }

                let boms: Vec<Dependency> = project_dm
                    .dependencies
                    .iter()
                    .filter(|(_, dep)| dep.scope.as_deref() == Some("import"))
                    .map(|(_, dep)| dep.clone())
                    .collect();

                for bom in boms {
                    trace!("got a BOM artifact: {}", bom.artifact_fqn);

                    // TODO add protection against infinite recursion
                    let bom_project =
                        self.build_effective_pom(&bom.artifact_fqn)?;

                    if let Some(DependencyManagement {
                        dependencies: bom_deps,
                    }) = bom_project.dependency_management
                    {
                        project_dm.dependencies.extend(bom_deps);
                    }
                }
            };

            return Ok(project);
        }

        Err(ResolverError::file_not_found(&format!("{}", project_id)))
    }

    pub fn fetch_project(
        &self,
        repository: &Repository,
        project_id: &Artifact,
    ) -> Result<Project, ResolverError>
    {
        // we're looking only for POMs here
        let project_id = project_id.with_packaging("pom");

        // check the cache first
        if let Some(cached_project) = self.project_cache.borrow().get(&project_id) {
            debug!("returning from cache {}...", project_id);
            return Ok(cached_project.clone());
        }

        // grab the remote POM
        let url = Self::create_url_with_repository(repository, &project_id)?;

        debug!("fetching {}...", url);
        let text = self.url_fetcher.fetch(&url)?;

        // parse the POM - it will be our "root" project
        // TODO handle multiple "roots"
        let mut project = self.pom_parser.parse(text)?;

        // make sure the packaging type is set to "pom"
        let mut project_id = project.artifact_fqn.with_packaging("pom");

        // TODO consider moving this to build_effective_pom
        // update the parent and fill-in the project's missing properties using the parent's GAV
        if let Some(parent) = &project.parent {
            let parent_fqn = parent.artifact_fqn.with_packaging("pom");

            project_id = project_id.normalize(&parent_fqn, "pom");

            // normalize dependency GAVs
            project.dependencies = normalize_gavs(project.dependencies, &parent_fqn, "jar");
            project.dependency_management = project.dependency_management.map(|mut dm| {
                dm.dependencies = normalize_gavs(dm.dependencies, &parent_fqn, "jar");
                dm
            });

            // save the updated FQN
            project.parent = project.parent.map(|mut p| {
                p.artifact_fqn = parent_fqn;
                p
            });
        }

        // save the updated FQN
        project.artifact_fqn = project_id.clone();

        // we're going to save all parsed projects into a HashMap
        // as a "cache"
        trace!("caching {}", project_id);
        self.project_cache
            .borrow_mut()
            .insert(project_id, project.clone());

        Ok(project)
    }

    pub fn download_all_jars(
        &self,
        root_artifacts: &[Artifact],
        root_directory: &Path,
    ) -> HashSet<Artifact>
    {
        let mut todo = VecDeque::new();
        todo.extend(root_artifacts.iter().cloned());

        let mut done = HashSet::new();

        while let Some(artifact) = todo.pop_front() {
            if !done.insert(artifact.clone()) {
                continue;
            }

            debug!("Resolving {}...", artifact);

            let project = self
                .build_effective_pom(&artifact)
                .unwrap();

            let _ = std::fs::create_dir_all(
                root_directory.join(project.artifact_fqn.artifact_id.as_ref().unwrap()),
            );

            let extract_path = PathBuf::from(
                root_directory.join(project.artifact_fqn.with_packaging("jar").filename()),
            );

            if !extract_path.exists() {
                let package = self
                    .try_download_package(&project.artifact_fqn)
                    .unwrap();

                package.extract_jar_file(&extract_path).unwrap();
            }

            for dep in project
                .dependencies
                .values()
                .filter(|dep| dep.scope.as_deref() == Some("compile"))
            {
                todo.push_back(dep.artifact_fqn.clone());
            }
        }

        done.into_iter().map(|a| a.with_packaging("jar")).collect()
    }
}
