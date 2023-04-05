async function search(prompt) {
    let requests = document.getElementById("results");
    requests.innerHTML = "";
    const response = await fetch("/api/search", {
        method: 'POST',
        headers: {'Content-Type': 'text/plain'},
        body: prompt,
    });

    let resultList = document.createElement("ul");
    resultList.className = "result-list";

    for ([path, rank] of await response.json()) {
        let item = document.createElement("li");

        let pathSpan = document.createElement("span");
        pathSpan.appendChild(document.createTextNode(path));
        item.appendChild(pathSpan);

        // 添加一个空格
        let space = document.createTextNode(" ");
        item.appendChild(space);

        let rankSpan = document.createElement("span");
        rankSpan.className = "rank";
        rankSpan.appendChild(document.createTextNode(rank));
        item.appendChild(rankSpan);

        resultList.appendChild(item);
    }
    requests.appendChild(resultList);
}

let query = document.getElementById("query");

query.addEventListener("keypress", (e) => {
    if (e.key == "Enter") {
        search(query.value);
    }
})