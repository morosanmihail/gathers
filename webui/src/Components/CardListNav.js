import React from "react";
import SelectionTracker from "./CardListNavButtons/SelectionTracker";
import DeleteCards from "./CardListNavButtons/DeleteCards";
import MoveCards from "./CardListNavButtons/MoveCards";
import ImportCards from "./CardListNavButtons/ImportCards";
import DeleteCollection from "./CardListNavButtons/DeleteCollection";
import ExportCollection from "./CardListNavButtons/ExportCollection";
import QuickSearch from "./CardListNavButtons/QuickSearch";
export default function CardListNav({ onToggleSearch, searchOpen }) {
  return (
    <nav
      className="navbar navbar-expand-md bg-body-tertiary"
      data-bs-theme="dark"
    >
      <div className="container-fluid">
        <QuickSearch onToggle={onToggleSearch} isOpen={searchOpen} />
        <SelectionTracker />
        <DeleteCards />
        <MoveCards />
        <ImportCards />
        <ExportCollection />
        <DeleteCollection />
      </div>
    </nav>
  );
}
