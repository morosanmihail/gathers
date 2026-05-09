import React from "react";
import { confirm } from "./ConfirmCollectionDelete";
import { useOperations } from "../../OperationsContext";
import { useCollection } from "../CollectionContext";
import {
  useSelectedCards,
  useSelectedCardsDispatch,
} from "../CardListContexts/SelectedCardsContext";
import { useRefreshCardList } from "../CardListContexts/RefreshCardListContext";

export default function DeleteCards() {
  const ops = useOperations();
  const collection = useCollection();
  const selected = useSelectedCards();
  const selectedDispatch = useSelectedCardsDispatch();
  const triggerRefresh = useRefreshCardList();

  const deleteCards = () => {
    confirm({ confirmType: "cards", selectedCount: selected.length }).then(
      ({ input }) => {
        Promise.all(
          selected.map((card) =>
            ops.fetch(
              "Removing " + card.id + " from " + collection,
              [],
              "/collection/cards/" + collection + "/delete",
              {
                method: "post",
                headers: {
                  Accept: "application/json",
                  "Content-Type": "application/json",
                },
                body: JSON.stringify({
                  id: card.id,
                  quantity: card.quantity,
                  foilQuantity: card.foilQuantity,
                }),
              },
            )
          )
        ).then(() => {
          triggerRefresh(true);
          selectedDispatch({ type: "empty" });
        });
      },
      () => {},
    );
  };

  return (
    <button onClick={deleteCards} type="button" className="btn btn-danger">
      🗑️
    </button>
  );
}
