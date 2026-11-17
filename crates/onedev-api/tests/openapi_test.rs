use onedev_api::models::pagination::{OffsetPagination, PaginatedResponse};
use onedev_api::openapi::create_openapi;

#[test]
fn openapi_document_contains_paths_and_schemas() {
    let doc = create_openapi();
    let json = serde_json::to_value(&doc).expect("openapi serializes");

    let paths = json["paths"].as_object().expect("paths present");
    assert!(paths.contains_key("/auth/login"));
    assert!(paths.contains_key("/projects/{id}"));
    assert!(paths.contains_key("/health"));

    let schemas = json["components"]["schemas"]
        .as_object()
        .expect("schemas present");
    assert!(schemas.contains_key("UserResponse"));
    assert!(schemas.contains_key("ProjectResponse"));
    assert!(schemas.contains_key("ProblemDetails"));

    assert_eq!(json["info"]["title"], "OneDev API");
}

#[test]
fn paginated_response_fills_total_and_pages() {
    let response = PaginatedResponse::new(
        vec![1, 2],
        45,
        OffsetPagination {
            page: 0,
            per_page: 20,
        },
    );

    assert_eq!(response.pagination.total, 45);
    assert_eq!(response.pagination.total_pages, Some(3));
    assert!(response.pagination.has_more);
}

#[test]
fn offset_pagination_default_is_first_page_with_default_limit() {
    let pagination = OffsetPagination::default();
    assert_eq!(pagination.page, 0);
    assert_eq!(pagination.per_page, 20);
}
