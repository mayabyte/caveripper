import * as dweevil from "dweevil";

var button = document.getElementById("cr-generate");
button.onclick = generate;

var sublevelInput = document.getElementById("cr-sublevel");
var seedInput = document.getElementById("cr-seed");
var currentSeedText = document.getElementById("cr-current-seed");
var queryInput = document.getElementById("cr-query");
var queryError = document.getElementById("cr-query-error");

// Generate a new image when Enter is pressed in the seed field
seedInput.addEventListener("keydown", function (e) {
    if (e.code === "Enter") {
        generate();
    }
});

// Run the query when Enter is pressed in the query box
queryInput.addEventListener("keydown", function (e) {
    if (e.code === "Enter") {
        query();
    }
});

// Generate a new image whenever the sublevel is changed
sublevelInput.onchange = generate;

async function generate() {
    var seed;
    if (seedInput.value === "") {
        seed = Math.floor(Math.random() * Math.pow(2, 32));
        currentSeedText.innerText = "0x" + seed.toString(16).toUpperCase();
    }
    else {
        seed = Number(seedInput.value);
        currentSeedText.innerText = "";
    }
    if (sublevelInput.value !== "") {
        var img = dweevil.cavegen(sublevelInput.value, seed);
        dweevil.draw_to_canvas(img, "cr-canvas");
    }
    queryError.hidden = true;
}

async function query() {
    if (queryInput.value !== "") {
        try {
            var img = dweevil.query(queryInput.value);
            dweevil.draw_to_canvas(img, "cr-canvas");
            queryError.hidden = true;
            currentSeedText.innerText = "";
        }
        catch {
            queryError.hidden = false;
        }
    }
    
}

// Start off by generating a layout
generate();
