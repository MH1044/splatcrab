% covers: 2 - the matmul result shape goes through the same guard as the broadcast one
% An outer product of two 1e5 vectors. Exit 1, never the allocator's 134.
ones(1e5, 1) * ones(1, 1e5)
