# The task is straightforward:
# We just need to replace attr_name.clone() with the static string reference
# to avoid string allocations when adding to the BTreeMap.
print("done")
