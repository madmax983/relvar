with open('relvar/src/tools/importer.rs', 'r') as f:
    content = f.read()

content = content.replace(
    '''    let parsed_headers: Vec<String> = parse_csv_line(header_line_trimmed, delimiter);
    let mut headers: Vec<Option<String>> = vec![None; parsed_headers.len()];

    // Validate headers. Map each CSV column index to the pre-allocated static attribute name from the Heading
    for (attr_name, _) in heading.attributes() {
        if let Some(pos) = parsed_headers.iter().position(|h| h == attr_name) {
            headers[pos] = Some(attr_name.clone());
        } else {
            return Err(ImporterError::MissingValue(format!(
                "Header missing attribute '{}'",
                attr_name
            )));
        }
    }''',
    '''    let parsed_headers: Vec<String> = parse_csv_line(header_line_trimmed, delimiter);
    let mut headers: Vec<Option<String>> = vec![None; parsed_headers.len()];

    // Validate headers. Map each CSV column index to the pre-allocated static attribute name from the Heading
    for (attr_name, _) in heading.attributes() {
        if let Some(pos) = parsed_headers.iter().position(|h| h == attr_name) {
            headers[pos] = Some(attr_name.clone());
        } else {
            return Err(ImporterError::MissingValue(format!(
                "Header missing attribute '{}'",
                attr_name
            )));
        }
    }'''
)

with open('relvar/src/tools/importer.rs', 'w') as f:
    f.write(content)
