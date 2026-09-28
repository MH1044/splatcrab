% covers: 14 - resolution starts with variables: a variable sq shadows the script's local function sq, so sq(2) indexes it
sq = [7 8];
disp(sq(2))
function y = sq(x)
    y = x^2;
end
