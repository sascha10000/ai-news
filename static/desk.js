(function () {
    // Date stamp in the masthead
    var el = document.getElementById('desk-date');
    if (el) {
        var now = new Date();
        var weekday = ['Sun','Mon','Tue','Wed','Thu','Fri','Sat'][now.getDay()];
        var month = ['Jan','Feb','Mar','Apr','May','Jun','Jul','Aug','Sep','Oct','Nov','Dec'][now.getMonth()];
        el.textContent = weekday + ', ' + month + ' ' + now.getDate() + ', ' + now.getFullYear();
    }

    // Contents-index tab routing — show one section at a time, sync URL hash
    var toc = document.querySelector('.desk-toc');
    var sections = document.querySelectorAll('.desk-content > .desk-section');
    if (!toc || !sections.length) return;

    function activate(id) {
        var found = false;
        sections.forEach(function (s) {
            var on = s.id === id;
            s.classList.toggle('is-active', on);
            if (on) found = true;
        });
        toc.querySelectorAll('a').forEach(function (a) {
            a.classList.toggle('is-active', a.getAttribute('href') === '#' + id);
        });
        return found;
    }

    function fromHash() {
        var h = (window.location.hash || '').replace('#', '');
        if (!h || !activate(h)) activate(sections[0].id);
    }

    toc.addEventListener('click', function (e) {
        var a = e.target.closest('a');
        if (!a) return;
        var id = (a.getAttribute('href') || '').replace('#', '');
        if (!id) return;
        e.preventDefault();
        history.replaceState(null, '', '#' + id);
        activate(id);
        window.scrollTo(0, 0);
    });

    window.addEventListener('hashchange', fromHash);
    fromHash();
})();
