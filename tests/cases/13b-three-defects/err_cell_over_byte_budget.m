% covers: 4 - a cell array past the 2 GB byte budget is a clean error in check_shape's message form
% 2^27 elements pass the element cap of 2^28 but cost far more than 2 GB.
c = cell(1, 2^27)
