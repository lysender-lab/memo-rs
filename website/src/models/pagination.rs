use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct PaginatedMeta {
    pub page: i64,
    pub per_page: i64,
    pub total_records: i64,
    pub total_pages: i64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Paginated<T> {
    pub meta: PaginatedMeta,
    pub data: Vec<T>,
}

#[derive(Clone, Serialize)]
pub struct PaginationLinks {
    pub prev: Option<PaginationLink>,
    pub next: Option<PaginationLink>,
    pub items: Vec<Option<PaginationLink>>,
}

#[derive(Clone, Serialize)]
pub struct PaginationLink {
    pub page: i64,
    pub url: String,
    pub active: bool,
}

impl PaginationLinks {
    pub fn new(meta: &PaginatedMeta, base_url: &str, suffix: &str) -> Self {
        let mut items = Vec::new();
        let mut prev = None;
        let mut next = None;

        let page = meta.page;
        let total_pages = meta.total_pages;

        // Identify the previous and next pages
        if page > 1 {
            prev = Some(PaginationLink {
                page: page - 1,
                url: format!(
                    "{}?page={}&per_page={}{}",
                    base_url,
                    page - 1,
                    meta.per_page,
                    suffix,
                ),
                active: false,
            });
        }

        if page < total_pages {
            next = Some(PaginationLink {
                page: page + 1,
                url: format!(
                    "{}?page={}&per_page={}{}",
                    base_url,
                    page + 1,
                    meta.per_page,
                    suffix,
                ),
                active: false,
            });
        }

        // Create 1 item to the left and to the right of the current page except
        // when there are not enough items

        // There are 3 scenarios:
        // 1. current page is page 1
        // 2. current page is at the middle
        // 3. current page is the last page

        let mut mid_start: Option<i64> = None;
        let mut mid_end: Option<i64> = None;

        // Do we need to render page 1 at all?
        if total_pages > 1 {
            items.push(Some(PaginationLink {
                page: 1,
                url: format!("{}?page=1&per_page={}{}", base_url, meta.per_page, suffix),
                active: 1 == page,
            }));

            if total_pages > 2 && total_pages <= 4 {
                // Just render all pages
                // In theory, they can be the same number
                mid_start = Some(2);
                mid_end = Some(total_pages - 1);
            }

            if total_pages > 4 {
                if page == 1 {
                    // Starting from first page, render + 2 pages to the right
                    mid_start = Some(page + 1);
                    mid_end = Some(page + 2);
                } else if page == total_pages {
                    // Starting from last page, render + 2 pages to the left
                    mid_start = Some(total_pages - 2);
                    // Do not include the last page since it will render on its own
                    mid_end = Some(total_pages - 1);
                } else {
                    if page == 2 {
                        mid_start = Some(page);
                    } else {
                        mid_start = Some(page - 1);
                    }
                    if page == total_pages - 1 {
                        mid_end = Some(page);
                    } else {
                        mid_end = Some(page + 1);
                    }
                }
            }
        }

        // Inject the middle pages
        match (mid_start, mid_end) {
            (Some(start), Some(end)) => {
                if start != 2 {
                    // Insert a blank page after the first page
                    items.push(None);
                }

                for i in start..=end {
                    items.push(Some(PaginationLink {
                        page: i,
                        url: format!(
                            "{}?page={}&per_page={}{}",
                            base_url, i, meta.per_page, suffix
                        ),
                        active: i == page,
                    }));
                }

                if end != total_pages - 1 {
                    // Insert a blank page before the last page
                    items.push(None);
                }
            }
            _ => {}
        }

        // Do we need to render the last page at all?
        if total_pages > 1 {
            items.push(Some(PaginationLink {
                page: total_pages,
                url: format!(
                    "{}?page={}&per_page={}{}",
                    base_url, total_pages, meta.per_page, suffix
                ),
                active: total_pages == page,
            }));
        }

        PaginationLinks { prev, next, items }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        let meta = PaginatedMeta {
            page: 1,
            per_page: 10,
            total_records: 0,
            total_pages: 0,
        };
        let links = PaginationLinks::new(&meta, "", "");
        assert!(links.prev.is_none());
        assert!(links.next.is_none());
        assert_eq!(links.items.len(), 0);
    }

    #[test]
    fn test_one_page() {
        let meta = PaginatedMeta {
            page: 1,
            per_page: 10,
            total_records: 5,
            total_pages: 1,
        };
        let links = PaginationLinks::new(&meta, "", "");
        assert!(links.prev.is_none());
        assert!(links.next.is_none());
        assert_eq!(links.items.len(), 0);
        assert!(links.items.iter().all(|x| x.is_some()));
    }

    #[test]
    fn test_two_pages() {
        let meta = PaginatedMeta {
            page: 1,
            per_page: 10,
            total_records: 20,
            total_pages: 2,
        };
        let links = PaginationLinks::new(&meta, "", "");
        assert!(links.prev.is_none());
        assert!(links.next.is_some());
        assert_eq!(links.items.len(), 2);
        assert!(links.items.iter().all(|x| x.is_some()));
    }

    #[test]
    fn test_three_pages() {
        let meta = PaginatedMeta {
            page: 1,
            per_page: 10,
            total_records: 30,
            total_pages: 3,
        };
        let links = PaginationLinks::new(&meta, "", "");
        assert!(links.prev.is_none());
        assert!(links.next.is_some());
        assert_eq!(links.items.len(), 3);
        assert!(links.items.iter().all(|x| x.is_some()));
    }

    #[test]
    fn test_page_3_of_3() {
        let meta = PaginatedMeta {
            page: 3,
            per_page: 10,
            total_records: 30,
            total_pages: 3,
        };
        let links = PaginationLinks::new(&meta, "", "");
        assert!(links.prev.is_some());
        assert!(links.next.is_none());
        assert_eq!(links.items.len(), 3);
        assert!(links.items.iter().all(|x| x.is_some()));
    }

    #[test]
    fn test_page_2_of_3() {
        let meta = PaginatedMeta {
            page: 2,
            per_page: 10,
            total_records: 30,
            total_pages: 3,
        };
        let links = PaginationLinks::new(&meta, "", "");
        assert!(links.prev.is_some());
        assert!(links.next.is_some());
        assert_eq!(links.items.len(), 3);
        assert!(links.items.iter().all(|x| x.is_some()));
    }

    #[test]
    fn test_page_1_of_4() {
        let meta = PaginatedMeta {
            page: 1,
            per_page: 10,
            total_records: 40,
            total_pages: 4,
        };
        let links = PaginationLinks::new(&meta, "", "");
        assert!(links.prev.is_none());
        assert!(links.next.is_some());
        assert_eq!(links.items.len(), 4);
        assert!(links.items.iter().all(|x| x.is_some()));
    }

    #[test]
    fn test_page_1_of_5() {
        let meta = PaginatedMeta {
            page: 1,
            per_page: 10,
            total_records: 50,
            total_pages: 5,
        };
        let links = PaginationLinks::new(&meta, "", "");
        assert!(links.prev.is_none());
        assert!(links.next.is_some());
        assert_eq!(links.items.len(), 5);
        assert!(links.items.get(3).unwrap().is_none());
    }

    #[test]
    fn test_page_1_of_6() {
        let meta = PaginatedMeta {
            page: 1,
            per_page: 10,
            total_records: 60,
            total_pages: 6,
        };
        let links = PaginationLinks::new(&meta, "", "");
        assert!(links.prev.is_none());
        assert!(links.next.is_some());
        assert_eq!(links.items.len(), 5);
        assert!(links.items.get(3).unwrap().is_none());
    }

    #[test]
    fn test_page_1_of_7() {
        let meta = PaginatedMeta {
            page: 1,
            per_page: 10,
            total_records: 70,
            total_pages: 7,
        };
        let links = PaginationLinks::new(&meta, "", "");
        assert!(links.prev.is_none());
        assert!(links.next.is_some());
        assert_eq!(links.items.len(), 5);
        assert!(links.items.get(3).unwrap().is_none());
    }

    #[test]
    fn test_page_1_of_100() {
        let meta = PaginatedMeta {
            page: 1,
            per_page: 10,
            total_records: 1000,
            total_pages: 100,
        };
        let links = PaginationLinks::new(&meta, "", "");
        assert!(links.prev.is_none());
        assert!(links.next.is_some());
        assert_eq!(links.items.len(), 5);
        assert!(links.items.get(3).unwrap().is_none());
    }

    #[test]
    fn test_page_4_of_4() {
        let meta = PaginatedMeta {
            page: 4,
            per_page: 10,
            total_records: 40,
            total_pages: 4,
        };
        let links = PaginationLinks::new(&meta, "", "");
        assert!(links.prev.is_some());
        assert!(links.next.is_none());
        assert_eq!(links.items.len(), 4);
    }

    #[test]
    fn test_page_5_of_5() {
        let meta = PaginatedMeta {
            page: 5,
            per_page: 10,
            total_records: 50,
            total_pages: 5,
        };
        let links = PaginationLinks::new(&meta, "", "");
        assert!(links.prev.is_some());
        assert!(links.next.is_none());
        assert_eq!(links.items.len(), 5);
        assert!(links.items.get(1).unwrap().is_none());
    }

    #[test]
    fn test_page_6_of_6() {
        let meta = PaginatedMeta {
            page: 6,
            per_page: 10,
            total_records: 60,
            total_pages: 6,
        };
        let links = PaginationLinks::new(&meta, "", "");
        assert!(links.prev.is_some());
        assert!(links.next.is_none());
        assert_eq!(links.items.len(), 5);
        assert!(links.items.get(1).unwrap().is_none());
    }

    #[test]
    fn test_page_100_of_100() {
        let meta = PaginatedMeta {
            page: 100,
            per_page: 10,
            total_records: 1000,
            total_pages: 100,
        };
        let links = PaginationLinks::new(&meta, "", "");
        assert!(links.prev.is_some());
        assert!(links.next.is_none());
        assert_eq!(links.items.len(), 5);
        assert!(links.items.get(1).unwrap().is_none());
    }

    #[test]
    fn test_page_2_of_5() {
        let meta = PaginatedMeta {
            page: 2,
            per_page: 10,
            total_records: 50,
            total_pages: 5,
        };
        let links = PaginationLinks::new(&meta, "", "");
        assert!(links.prev.is_some());
        assert!(links.next.is_some());
        assert_eq!(links.items.len(), 5);
        assert!(links.items.get(3).unwrap().is_none());
    }

    #[test]
    fn test_page_3_of_5() {
        let meta = PaginatedMeta {
            page: 3,
            per_page: 10,
            total_records: 50,
            total_pages: 5,
        };
        let links = PaginationLinks::new(&meta, "", "");
        assert!(links.prev.is_some());
        assert!(links.next.is_some());
        assert_eq!(links.items.len(), 5);
    }

    #[test]
    fn test_page_3_of_6() {
        let meta = PaginatedMeta {
            page: 3,
            per_page: 10,
            total_records: 60,
            total_pages: 6,
        };
        let links = PaginationLinks::new(&meta, "", "");
        assert!(links.prev.is_some());
        assert!(links.next.is_some());
        assert_eq!(links.items.len(), 6);
    }

    #[test]
    fn test_page_4_of_6() {
        let meta = PaginatedMeta {
            page: 4,
            per_page: 10,
            total_records: 60,
            total_pages: 6,
        };
        let links = PaginationLinks::new(&meta, "", "");
        assert!(links.prev.is_some());
        assert!(links.next.is_some());
        assert_eq!(links.items.len(), 6);
    }

    #[test]
    fn test_page_5_of_6() {
        let meta = PaginatedMeta {
            page: 5,
            per_page: 10,
            total_records: 60,
            total_pages: 6,
        };
        let links = PaginationLinks::new(&meta, "", "");
        assert!(links.prev.is_some());
        assert!(links.next.is_some());
        assert_eq!(links.items.len(), 5);
    }

    #[test]
    fn test_page_4_of_7() {
        let meta = PaginatedMeta {
            page: 4,
            per_page: 10,
            total_records: 70,
            total_pages: 7,
        };
        let links = PaginationLinks::new(&meta, "", "");
        assert!(links.prev.is_some());
        assert!(links.next.is_some());
        assert_eq!(links.items.len(), 7);
    }

    #[test]
    fn test_page_3_of_7() {
        let meta = PaginatedMeta {
            page: 3,
            per_page: 10,
            total_records: 70,
            total_pages: 7,
        };
        let links = PaginationLinks::new(&meta, "", "");
        assert!(links.prev.is_some());
        assert!(links.next.is_some());
        assert_eq!(links.items.len(), 6);
    }

    #[test]
    fn test_page_2_of_7() {
        let meta = PaginatedMeta {
            page: 2,
            per_page: 10,
            total_records: 70,
            total_pages: 7,
        };
        let links = PaginationLinks::new(&meta, "", "");
        assert!(links.prev.is_some());
        assert!(links.next.is_some());
        assert_eq!(links.items.len(), 5);
    }

    #[test]
    fn test_page_7_of_7() {
        let meta = PaginatedMeta {
            page: 7,
            per_page: 10,
            total_records: 70,
            total_pages: 7,
        };
        let links = PaginationLinks::new(&meta, "", "");
        assert!(links.prev.is_some());
        assert!(links.next.is_none());
        assert_eq!(links.items.len(), 5);
    }

    #[test]
    fn test_page_6_of_7() {
        let meta = PaginatedMeta {
            page: 6,
            per_page: 10,
            total_records: 70,
            total_pages: 7,
        };
        let links = PaginationLinks::new(&meta, "", "");
        assert!(links.prev.is_some());
        assert!(links.next.is_some());
        assert_eq!(links.items.len(), 5);
    }

    #[test]
    fn test_page_5_of_7() {
        let meta = PaginatedMeta {
            page: 5,
            per_page: 10,
            total_records: 70,
            total_pages: 7,
        };
        let links = PaginationLinks::new(&meta, "", "");
        assert!(links.prev.is_some());
        assert!(links.next.is_some());
        assert_eq!(links.items.len(), 6);
    }
}
