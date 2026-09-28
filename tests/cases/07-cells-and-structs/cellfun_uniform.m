% covers: 8 - cellfun calls a function on each element's content and returns the array of results
disp(cellfun(@numel, {'ab', 'cde', ''}))
