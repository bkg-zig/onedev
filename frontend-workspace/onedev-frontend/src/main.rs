//! OneDev Frontend - Dioxus SPA Entry Point

use dioxus::prelude::*;
use dioxus_router::*;

fn main() {
    dioxus::launch(App);
}

#[derive(Routable, Clone, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(Navbar)]
    #[route("/")]
    Dashboard {},
    #[route("/login")]
    Login {},
    #[route("/register")]
    Register {},
    #[route("/projects")]
    Projects {},
    #[route("/projects/:id")]
    ProjectDetail { id: String },
    #[route("/projects/new")]
    NewProject {},
    #[route("/projects/:project_id/issues")]
    Issues { project_id: String },
    #[route("/projects/:project_id/issues/new")]
    NewIssue { project_id: String },
    #[route("/projects/:project_id/issues/:id")]
    IssueDetail { project_id: String, id: String },
    #[route("/projects/:project_id/merge-requests")]
    MergeRequests { project_id: String },
    #[route("/projects/:project_id/merge-requests/new")]
    NewMR { project_id: String },
    #[route("/projects/:project_id/merge-requests/:id")]
    MRDetail { project_id: String, id: String },
    #[route("/projects/:project_id/builds")]
    Builds { project_id: String },
    #[route("/projects/:project_id/builds/:id")]
    BuildDetail { project_id: String, id: String },
    #[route("/projects/:project_id/git")]
    GitOverview { project_id: String },
    #[route("/profile")]
    Profile {},
    #[route("/profile/settings")]
    Settings {},
    #[route("/admin")]
    AdminDashboard {},
    #[route("/admin/users")]
    AdminUsers {},
    #[route("/admin/projects")]
    AdminProjects {},
    #[route("/admin/system")]
    AdminSystem {},
}

fn App() -> Element {
    rsx! {
        Router::<Route> {}
    }
}

#[component]
fn Navbar() -> Element {
    rsx! {
        nav { class: "navbar",
            div { class: "navbar-brand",
                Link { to: Route::Dashboard {}, "OneDev" }
            }
            div { class: "navbar-menu",
                Link { to: Route::Dashboard {}, "Dashboard" }
                Link { to: Route::Projects {}, "Projects" }
                Link { to: Route::AdminDashboard {}, "Admin" }
            }
        }
        Outlet::<Route> {}
    }
}

#[component]
fn Dashboard() -> Element {
    rsx! {
        div { class: "dashboard",
            h1 { "Dashboard" }
            p { "Welcome to OneDev Dashboard" }
        }
    }
}

#[component]
fn Login() -> Element {
    rsx! {
        div { class: "login",
            h1 { "Login" }
            form { class: "login-form",
                div { class: "form-group",
                    label { "Username" }
                    input { r#type: "text", name: "username" }
                }
                div { class: "form-group",
                    label { "Password" }
                    input { r#type: "password", name: "password" }
                }
                button { r#type: "submit", "Login" }
            }
        }
    }
}

#[component]
fn Register() -> Element {
    rsx! {
        div { class: "register",
            h1 { "Register" }
            form { class: "register-form",
                div { class: "form-group",
                    label { "Username" }
                    input { r#type: "text", name: "username" }
                }
                div { class: "form-group",
                    label { "Email" }
                    input { r#type: "email", name: "email" }
                }
                div { class: "form-group",
                    label { "Password" }
                    input { r#type: "password", name: "password" }
                }
                button { r#type: "submit", "Register" }
            }
        }
    }
}

#[component]
fn Projects() -> Element {
    rsx! {
        div { class: "projects",
            h1 { "Projects" }
            Link { to: Route::NewProject {}, class: "btn btn-primary", "New Project" }
            ul { class: "project-list",
                li { "Project 1" }
                li { "Project 2" }
                li { "Project 3" }
            }
        }
    }
}

#[component]
fn ProjectDetail(id: String) -> Element {
    rsx! {
        div { class: "project-detail",
            h1 { "Project: {id}" }
            nav { class: "project-tabs",
                Link { to: Route::Issues { project_id: id.clone() }, "Issues" }
                Link { to: Route::MergeRequests { project_id: id.clone() }, "Merge Requests" }
                Link { to: Route::Builds { project_id: id.clone() }, "Builds" }
                Link { to: Route::GitOverview { project_id: id.clone() }, "Git" }
            }
            Outlet::<Route> {}
        }
    }
}

#[component]
fn NewProject() -> Element {
    rsx! {
        div { class: "new-project",
            h1 { "New Project" }
            form { class: "project-form",
                div { class: "form-group",
                    label { "Name" }
                    input { r#type: "text", name: "name" }
                }
                div { class: "form-group",
                    label { "Description" }
                    textarea { name: "description" }
                }
                button { r#type: "submit", "Create Project" }
            }
        }
    }
}

#[component]
fn Issues(project_id: String) -> Element {
    rsx! {
        div { class: "issues",
            h1 { "Issues for Project: {project_id}" }
            Link { to: Route::NewIssue { project_id: project_id.clone() }, class: "btn btn-primary", "New Issue" }
            ul { class: "issue-list",
                li { "Issue 1" }
                li { "Issue 2" }
                li { "Issue 3" }
            }
        }
    }
}

#[component]
fn NewIssue(project_id: String) -> Element {
    rsx! {
        div { class: "new-issue",
            h1 { "New Issue for Project: {project_id}" }
            form { class: "issue-form",
                div { class: "form-group",
                    label { "Title" }
                    input { r#type: "text", name: "title" }
                }
                div { class: "form-group",
                    label { "Description" }
                    textarea { name: "description" }
                }
                button { r#type: "submit", "Create Issue" }
            }
        }
    }
}

#[component]
fn IssueDetail(project_id: String, id: String) -> Element {
    rsx! {
        div { class: "issue-detail",
            h1 { "Issue #{id} in Project {project_id}" }
        }
    }
}

#[component]
fn MergeRequests(project_id: String) -> Element {
    rsx! {
        div { class: "merge-requests",
            h1 { "Merge Requests for Project: {project_id}" }
            Link { to: Route::NewMR { project_id: project_id.clone() }, class: "btn btn-primary", "New MR" }
            ul { class: "mr-list",
                li { "MR 1" }
                li { "MR 2" }
            }
        }
    }
}

#[component]
fn NewMR(project_id: String) -> Element {
    rsx! {
        div { class: "new-mr",
            h1 { "New Merge Request for Project: {project_id}" }
            form { class: "mr-form",
                div { class: "form-group",
                    label { "Title" }
                    input { r#type: "text", name: "title" }
                }
                div { class: "form-group",
                    label { "Source Branch" }
                    input { r#type: "text", name: "source_branch" }
                }
                div { class: "form-group",
                    label { "Target Branch" }
                    input { r#type: "text", name: "target_branch" }
                }
                button { r#type: "submit", "Create MR" }
            }
        }
    }
}

#[component]
fn MRDetail(project_id: String, id: String) -> Element {
    rsx! {
        div { class: "mr-detail",
            h1 { "MR #{id} in Project {project_id}" }
        }
    }
}

#[component]
fn Builds(project_id: String) -> Element {
    rsx! {
        div { class: "builds",
            h1 { "Builds for Project: {project_id}" }
            ul { class: "build-list",
                li { "Build 1" }
                li { "Build 2" }
            }
        }
    }
}

#[component]
fn BuildDetail(project_id: String, id: String) -> Element {
    rsx! {
        div { class: "build-detail",
            h1 { "Build #{id} in Project {project_id}" }
        }
    }
}

#[component]
fn GitOverview(project_id: String) -> Element {
    rsx! {
        div { class: "git-overview",
            h1 { "Git for Project: {project_id}" }
        }
    }
}

#[component]
fn Profile() -> Element {
    rsx! {
        div { class: "profile",
            h1 { "Profile" }
        }
    }
}

#[component]
fn Settings() -> Element {
    rsx! {
        div { class: "settings",
            h1 { "Settings" }
        }
    }
}

#[component]
fn AdminDashboard() -> Element {
    rsx! {
        div { class: "admin-dashboard",
            h1 { "Admin Dashboard" }
            nav { class: "admin-nav",
                Link { to: Route::AdminUsers {}, "Users" }
                Link { to: Route::AdminProjects {}, "Projects" }
                Link { to: Route::AdminSystem {}, "System" }
            }
        }
    }
}

#[component]
fn AdminUsers() -> Element {
    rsx! {
        div { class: "admin-users",
            h1 { "User Management" }
        }
    }
}

#[component]
fn AdminProjects() -> Element {
    rsx! {
        div { class: "admin-projects",
            h1 { "Project Management" }
        }
    }
}

#[component]
fn AdminSystem() -> Element {
    rsx! {
        div { class: "admin-system",
            h1 { "System Info" }
        }
    }
}