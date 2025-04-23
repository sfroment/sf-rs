import { useState, useRef } from "react";
// import reactLogo from "./assets/react.svg"; // Removed
// import viteLogo from "/vite.svg"; // Removed
import "./App.css";

// Type for the messages state: can hold parsed JSON objects or raw strings
type MessageItem = object | string;

function App() {
  const [url, setUrl] = useState("ws://127.0.0.1:9999/ws?peer_id"); // Default URL
  const [isConnected, setIsConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [messages, setMessages] = useState<MessageItem[]>([]); // State for messages (objects or strings)
  const [messageToSend, setMessageToSend] = useState(""); // State for input message
  const ws = useRef<WebSocket | null>(null); // Use useRef to hold the WebSocket instance

  const handleConnect = () => {
    setError(null);
    setMessages([]); // Clear messages on new connection attempt
    console.log("Attempting to connect to:", url);
    try {
      // Close existing connection if any
      if (ws.current && ws.current.readyState === WebSocket.OPEN) {
        ws.current.close();
      }

      const socket = new WebSocket(url);
      ws.current = socket; // Store the WebSocket instance in ref

      socket.onopen = () => {
        console.log("WebSocket connection established");
        setIsConnected(true);
      };

      socket.onmessage = (event) => {
        console.log("Message from server:", event.data);
        try {
          // Try to parse the incoming data as JSON
          const parsedData = JSON.parse(event.data);
          setMessages((prevMessages) => [...prevMessages, parsedData]);
        } catch /* (parseError) */ {
          // If parsing fails, store the raw string
          // console.warn("Received non-JSON message:", event.data);
          setMessages((prevMessages) => [...prevMessages, event.data]);
        }
      };

      socket.onerror = (event) => {
        console.error("WebSocket error:", event);
        setError("WebSocket connection error. Check console for details.");
        setIsConnected(false);
        ws.current = null; // Clear ref on error
      };

      socket.onclose = (event) => {
        console.log("WebSocket connection closed:", event.code, event.reason);
        setIsConnected(false);
        ws.current = null; // Clear ref on close
        if (!event.wasClean) {
          setError(
            `Connection closed unexpectedly: ${
              event.reason || "Unknown reason"
            }`
          );
        }
        // setMessages([]); // Messages are cleared on new connect attempt or manual disconnect
      };
    } catch (err) {
      console.error("Failed to create WebSocket:", err);
      setError(
        err instanceof Error ? err.message : "An unknown error occurred"
      );
      setIsConnected(false);
      ws.current = null; // Clear ref on catch
    }
  };

  const handleDisconnect = () => {
    if (ws.current) {
      ws.current.close(); // Close the connection using the stored instance
      ws.current = null;
    }
    console.log("Disconnecting (manual)");
    setIsConnected(false);
    setError(null);
    setMessages([]); // Clear messages on manual disconnect
  };

  // Function to send a message
  const handleSendMessage = () => {
    if (
      ws.current &&
      ws.current.readyState === WebSocket.OPEN &&
      messageToSend
    ) {
      console.log("Sending message:", messageToSend);
      ws.current.send(messageToSend);
      setMessageToSend(""); // Clear input after sending
    } else if (!ws.current || ws.current.readyState !== WebSocket.OPEN) {
      setError("Cannot send message: WebSocket is not connected.");
    } else {
      // Optionally provide feedback if message is empty
      console.log("Cannot send empty message");
    }
  };

  return (
    <>
      <h1>WebSocket Client</h1>
      <div className="connection-form">
        <label htmlFor="ws-url">WebSocket URL:</label>
        <input
          type="text"
          id="ws-url"
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          disabled={isConnected} // Disable input when connected
        />
        {!isConnected ? (
          <button onClick={handleConnect}>Connect</button>
        ) : (
          <button onClick={handleDisconnect}>Disconnect</button>
        )}
      </div>
      {error && <p style={{ color: "red" }}>Error: {error}</p>}
      <p>Status: {isConnected ? "Connected" : "Disconnected"}</p>

      {/* Area to send messages */}
      <div className="send-message-form">
        <input
          type="text"
          value={messageToSend}
          onChange={(e) => setMessageToSend(e.target.value)}
          placeholder="Enter message to send"
          disabled={!isConnected} // Disable if not connected
          onKeyDown={(e) => {
            // Allow sending with Enter key
            if (e.key === "Enter") {
              handleSendMessage();
            }
          }}
        />
        <button
          onClick={handleSendMessage}
          disabled={!isConnected || !messageToSend}
        >
          Send
        </button>
      </div>

      {/* Area to display received messages */}
      <div className="messages-area">
        <h2>Received Messages:</h2>
        <pre className="message-box">
          {messages.map((msg, index) => (
            <div key={index}>
              {typeof msg === "object" ? JSON.stringify(msg, null, 2) : msg}
            </div>
          ))}
          {messages.length === 0 && <p>No messages received yet.</p>}
        </pre>
      </div>
    </>
  );
}

export default App;
