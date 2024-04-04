import * as dweevil from "dweevil";

var button = document.getElementById("new-layout-button");
button.onclick = run;

var sublevelInput = document.getElementById("sublevel-input");
var seedInput = document.getElementById("seed-input");

async function run() {
    var seed;
    if (seedInput.value === "") {
        seed = Math.floor(Math.random() * Math.pow(2, 32));
    }
    else {
        seed = Number(seedInput.value);
    }
    if (sublevelInput.value !== "") {
        var img = dweevil.cavegen(sublevelInput.value, seed);
        dweevil.draw_to_canvas(img, "canvas");
    }
}
run(); // Execute async wrapper 
