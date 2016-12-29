+function() {

    var ws = new WebSocket("ws://" + location.host + "/")
    var mb = document.getElementById('responses');
    var input = document.getElementById('input');
    var my_user_id = null;
    ws.onopen = function() {
        log('debug', "Connected")
        input.style.visibility = 'visible'
    }

    ws.onclose = function() {
        input.style.visibility = 'hidden'
        log('warning', "Disconnected")
    }

    ws.onerror = function(e) {
        input.style.visibility = 'hidden'
        log('warning', 'ERROR: ' + e)
    }
    ws.onmessage = function(ev) {
        log('text', ev.data);
    }
    input.onkeydown = function(ev) {
        if(ev.which == 13) {
            ws.send(input.value);
            input.value = ''
        }
    }


    function log(type, message) {
        let red = document.createElement('div');
        red.className = type;
        red.appendChild(document.createTextNode(message));
        mb.insertBefore(red, mb.childNodes[0]);
    }

}()
