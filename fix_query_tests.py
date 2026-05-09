with open("relvar-core/src/query/mod.rs", "r") as f:
    content = f.read()

content = content.replace('.project(vec!["name", "email"]);', '.project(["name", "email"]);')
content = content.replace('.rename(vec![("name", "full_name")]);', '.rename([("name", "full_name")]);')
content = content.replace('.summarize(vec!["department"], vec![Aggregation::count("emp_count")]);', '.summarize(["department"], [Aggregation::count("emp_count")]);')

content = content.replace('.project(vec!["name"]);', '.project(["name"]);')
content = content.replace('.rename(vec![("name", "full_name")]);', '.rename([("name", "full_name")]);')
content = content.replace('.summarize(vec!["age"], vec![Aggregation::count("count")]);', '.summarize(["age"], [Aggregation::count("count")]);')
content = content.replace('.rename(vec![("old", "new")]);', '.rename([("old", "new")]);')
content = content.replace('.summarize(vec!["a"], vec![Aggregation::count("cnt")]);', '.summarize(["a"], [Aggregation::count("cnt")]);')
content = content.replace('.summarize(vec!["nonexistent_col"], vec![]);', '.summarize(["nonexistent_col"], []);')

with open("relvar-core/src/query/mod.rs", "w") as f:
    f.write(content)
