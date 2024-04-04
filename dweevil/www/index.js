import * as dweevil from "dweevil";

var button = document.getElementById("new-layout-button");
button.onclick = run;

var seed = 0x1234ABCC;
async function run() {
    seed += 1;
    var img = dweevil.cavegen("sh6", seed);
    dweevil.draw_to_canvas(img, "canvas");
}
run(); // Execute async wrapper 
