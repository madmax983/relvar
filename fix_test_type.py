with open('relvar-storage/benches/mvcc.rs', 'r') as f:
    lines = f.readlines()
lines[47] = lines[47].replace('|(mut engine, _temp_dir): (PersistentEngine, _)|', '|(engine, _temp_dir): (PersistentEngine, _)|')
lines[173] = lines[173].replace('|(mut engine, _temp_dir): (PersistentEngine, _)|', '|(engine, _temp_dir): (PersistentEngine, _)|')
with open('relvar-storage/benches/mvcc.rs', 'w') as f:
    f.writelines(lines)
