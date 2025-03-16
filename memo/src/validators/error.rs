use validator::{ValidationError, ValidationErrors};

pub fn flatten_errors(errors: &ValidationErrors) -> String {
    // Collect field keys first
    let mut fields: Vec<String> = errors
        .field_errors()
        .keys()
        .map(|k| k.to_string())
        .collect();

    // Ensure error fields are sorted ascending
    fields.sort();

    let field_errors = errors.field_errors();
    let messages: Vec<String> = fields
        .into_iter()
        .map(|k| {
            let Some(item) = field_errors.get(k.as_str()) else {
                return format!("{}: invalid", k);
            };
            let msgs: Vec<String> = item.iter().map(|i| error_to_string(i)).collect();
            format!("{}: {}", k, msgs.join(", "))
        })
        .collect();

    messages.join(", ")
}

fn error_to_string(error: &ValidationError) -> String {
    // Provide partial error code conversion
    match error.code.as_ref() {
        "email" => "invalid email".to_string(),
        "url" => "invalid url".to_string(),
        "length" => match (
            error.params.get("min"),
            error.params.get("max"),
            error.params.get("equal"),
        ) {
            (Some(min), Some(max), None) => {
                format!("must be between {} and {} characters", min, max)
            }
            (Some(min), None, None) => format!("must be at least {} characters", min),
            (None, Some(max), None) => format!("must be at most {} characters", max),
            (None, None, Some(equal)) => format!("must be be {} characters", equal),
            _ => "invalid length".to_string(),
        },
        "range" => match (error.params.get("min"), error.params.get("max")) {
            (Some(min), Some(max)) => format!("must be between {} and {}", min, max),
            (Some(min), None) => format!("must be at least {}", min),
            (None, Some(max)) => format!("must be at most {}", max),
            _ => "invalid".to_string(),
        },
        "required" => "required".to_string(),
        "sluggable" => "must be composed of alpha-numeric characters or dashes".to_string(),
        _ => "invalid".to_string(),
    }
}
