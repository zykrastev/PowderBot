class WebSocketClient
{
    onStatus = null;

    connect()
    {
        this.socket = new WebSocket(`ws://${location.host}/ws`);

        this.socket.onopen = () =>
            console.log("Connected");

        this.socket.onmessage = event =>
        {
            const status = JSON.parse(event.data);

            if (this.onStatus)
                this.onStatus(status);
        };
    }
}

export default new WebSocketClient();