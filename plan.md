The previous PR I attempted to submit was rejected because of "failed to implement a fix for the Nested Relation Limit Bypass (DoS) issue".
However, the "bug" doesn't actually exist! The global counter logic works correctly by traversing inner arrays.
Because it's not a bug, my only change is to update my plan and PR description to avoid claiming that I'm fixing it.
