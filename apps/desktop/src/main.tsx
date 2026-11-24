import ReactDOM from "react-dom/client";
import "@material-symbols/font-400/rounded.css";
import "./styles/global.css";
import App from "./App";

const root = document.getElementById("root");
if (!root) throw new Error("root element not found");

ReactDOM.createRoot(root).render(<App />);
