import sys

with open("relvar/tests/sentry_timeseries_collision.rs", "r") as f:
    content = f.read()

content = content.replace("use relvar::moving_average;", "use relvar::experimental::timeseries::moving_average;")

with open("relvar/tests/sentry_timeseries_collision.rs", "w") as f:
    f.write(content)
